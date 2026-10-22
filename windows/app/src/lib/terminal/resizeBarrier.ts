// xterm 크기를 ConPTY 의 다시 그리기 머리말(cut)에 맞춰 바꾸는 상태 기계.
// Idle → Sending(termResize 의 기준 seq 를 기다림) → Scanning(스캐너에 출력을 먹임) → Holding(cut 앞을 쓰고 콜백에서 applyLocal)
// → 붙잡은 프레임을 풀고 → 다음 단계 또는 최신 요청으로 다시 계획 → Idle.
// 한 번에 단계 하나만 진행하고, 요청은 슬롯 하나(desired)에 합친다. 디바운스는 호출자(term.ts)의 몫이다.
// 붙잡은 프레임은 xterm 에 쓰기 전까지 done 을 부르지 않는다(생산자의 크레딧 창이 막아 준다).
// 이 파일은 resizeRules.ts·repaintScanner.ts 만 import 하고 타이머는 port 로 주입받아 node:test 로 바로 시험한다.

import { RepaintScanner } from "./repaintScanner.ts";
import { planSteps, type Size, type Step } from "./resizeRules.ts";

export interface BarrierPort {
  /** termResize — 적용했으면 ConPTY 크기 변경 직전의 스트림 위치, 적용 안 했으면 null */
  sendResize(size: Size): Promise<number | null>;
  /** xterm 크기 바꾸기 */
  applyLocal(size: Size): void;
  write(bytes: Uint8Array, done: () => void): void;
  current(): Size;
  altScreen(): boolean;
  bound(): boolean;
  setTimer(fn: () => void, ms: number): unknown;
  clearTimer(handle: unknown): void;
  log(message: string): void;
}

export interface BarrierOptions {
  deadlineMs?: number;
  tailIdleMs?: number;
  scanBudget?: number;
}

export const PREAMBLE_DEADLINE_MS = 1000;
export const TAIL_IDLE_MS = 50;
export const SCAN_BUDGET_BYTES = 64 * 1024;

type Phase = "idle" | "sending" | "scanning" | "holding";
interface Held {
  seq: number;
  bytes: Uint8Array;
  done: () => void;
}

const NO_BYTES = new Uint8Array(0);
const noop = () => {};

export class ResizeBarrier {
  private readonly deadlineMs: number;
  private readonly tailIdleMs: number;
  private readonly scanBudget: number;

  private phase: Phase = "idle";
  private desired: Size | null = null;
  private steps: Step[] = [];
  private stepIndex = 0;
  private minSeq: number | undefined;
  private token = 0; // 단계마다 올린다: 늦게 도착한 약속·콜백이 지난 단계를 건드리지 못하게
  private disposed = false;

  private scanner: RepaintScanner | null = null;
  private anchor = 0;
  private lastEnd: number | null = null; // 스캐너에 먹인 마지막 끝 seq
  private scannedBytes = 0;
  private fallback = false;
  private queue: Held[] = [];
  private deadlineTimer: unknown = null;
  private idleTimer: unknown = null;

  private readonly port: BarrierPort;

  constructor(port: BarrierPort, options: BarrierOptions = {}) {
    this.port = port;
    this.deadlineMs = options.deadlineMs ?? PREAMBLE_DEADLINE_MS;
    this.tailIdleMs = options.tailIdleMs ?? TAIL_IDLE_MS;
    this.scanBudget = options.scanBudget ?? SCAN_BUDGET_BYTES;
  }

  /** 원하는 크기. 진행 중이면 슬롯에 덮어쓰고, 끝난 뒤 한 번만 다시 계획한다 */
  request(desired: Size): void {
    if (this.disposed) return;
    if (!this.port.bound()) {
      // 붙기 전에는 보낼 곳이 없다: xterm 만 바꾸고 상태는 두지 않는다
      const now = this.port.current();
      if (now.cols !== desired.cols || now.rows !== desired.rows) this.port.applyLocal(desired);
      return;
    }
    this.desired = desired;
    if (this.phase === "idle") this.beginPlan();
  }

  /** 데이터 프레임 하나. 크기를 바꾸는 중이 아니면 그대로 쓴다 */
  output(seq: number, bytes: Uint8Array, done: () => void): void {
    if (this.disposed) return;
    if (bytes.length === 0) {
      done();
      return;
    }
    if (this.phase === "idle") {
      this.port.write(bytes, done);
      return;
    }
    this.queue.push({ seq, bytes, done });
    if (this.phase === "scanning") this.feedScanner(seq, bytes);
  }

  /** 스트림이 끊겼다(gap)·프로세스가 끝났다(exit): 기다림을 버리고 계획한 크기를 xterm 에 바로 적용한다 */
  interrupt(reason: "gap" | "exit"): void {
    if (this.disposed) return;
    if (reason === "exit") this.desired = null;
    // 이미 cut 앞을 쓰는 중이면 콜백이 곧 적용하고 푼다
    if (this.phase === "idle" || this.phase === "holding") return;
    this.port.log(`resize barrier: ${reason} 로 기다림을 버리고 바로 적용`);
    this.token++;
    this.clearTimers();
    this.scanner = null;
    this.port.applyLocal(this.steps[this.stepIndex].to);
    this.drain(Infinity);
    this.phase = "idle";
    if (this.desired !== null) this.beginPlan();
  }

  /** 타이머를 걷고 이후 port 를 부르지 않는다 */
  dispose(): void {
    this.disposed = true;
    this.token++;
    this.clearTimers();
    this.scanner = null;
    this.queue = [];
    this.desired = null;
  }

  // ---- 계획 ----

  private beginPlan(): void {
    const want = this.desired;
    this.desired = null;
    if (want === null) return;
    if (!this.port.bound()) {
      this.port.applyLocal(want);
      return;
    }
    const steps = planSteps(this.port.current(), want, this.port.altScreen());
    if (steps.length === 0) return;
    this.steps = steps;
    this.stepIndex = 0;
    this.minSeq = undefined;
    this.sendStep();
  }

  private sendStep(): void {
    const step = this.steps[this.stepIndex];
    const token = ++this.token;
    if (!this.port.bound()) {
      this.phase = "holding";
      this.completeStep(token);
      return;
    }
    this.phase = "sending";
    this.scanner = null;
    this.lastEnd = null;
    this.scannedBytes = 0;
    this.fallback = false;
    let pending: Promise<number | null>;
    try {
      pending = this.port.sendResize(step.to);
    } catch (error) {
      pending = Promise.reject(error);
    }
    pending.then(
      (anchor) => this.onAnchor(token, anchor),
      (error) => this.onRejected(token, error),
    );
  }

  private onRejected(token: number, error: unknown): void {
    if (token !== this.token || this.disposed) return;
    this.port.log(`resize barrier: termResize 실패, 바로 적용 (${String(error)})`);
    this.applyNow(token);
  }

  private onAnchor(token: number, anchor: number | null): void {
    if (token !== this.token || this.disposed) return;
    if (anchor === null) {
      this.applyNow(token); // 적용 안 됨(같은 크기·끝남): 기다릴 머리말이 없다
      return;
    }
    const step = this.steps[this.stepIndex];
    this.anchor = anchor;
    this.scanner = new RepaintScanner({
      target: step.to,
      cut: step.cut,
      alt: this.port.altScreen(),
      anchorSeq: anchor,
      minSeq: this.minSeq,
    });
    this.phase = "scanning";
    this.deadlineTimer = this.port.setTimer(() => this.onDeadline(token), this.deadlineMs);
    for (const held of [...this.queue]) {
      if (this.phase !== "scanning" || token !== this.token) return;
      this.feedScanner(held.seq, held.bytes);
    }
  }

  private applyNow(token: number): void {
    this.phase = "holding";
    this.completeStep(token); // 기다릴 머리말이 없다: 바꾼 뒤 붙잡은 것을 푼다
  }

  // ---- 스캔 ----

  private feedScanner(seq: number, bytes: Uint8Array): void {
    const scanner = this.scanner as RepaintScanner;
    const end = seq + bytes.length;
    this.scannedBytes += Math.max(0, end - Math.max(seq, this.anchor));
    this.lastEnd = Math.max(this.lastEnd ?? end, end);
    const result = scanner.feed(seq, bytes);
    this.afterFeed(result);
  }

  private afterFeed(result: { safe: number; cut: number | null }): void {
    if (result.cut !== null) {
      this.holdAtCut(result.cut);
      return;
    }
    this.drain(result.safe);
    if (!this.fallback && this.scannedBytes >= this.scanBudget) {
      this.enterFallback("64KiB 안에 머리말이 없다");
      return;
    }
    const heldTail = this.lastEnd !== null && result.safe < this.lastEnd;
    if (this.fallback || heldTail) this.armIdle();
    else this.clearIdle();
  }

  private onDeadline(token: number): void {
    this.deadlineTimer = null;
    if (token !== this.token || this.phase !== "scanning" || this.fallback) return;
    this.enterFallback(`머리말이 ${this.deadlineMs}ms 안에 안 왔다`);
  }

  /** 머리말을 포기하고 다음 LF 뒤에서 자른다 */
  private enterFallback(reason: string): void {
    this.port.log(`resize barrier: ${reason}, 줄 끝에서 자른다`);
    this.fallback = true;
    const scanner = this.scanner as RepaintScanner;
    scanner.lineFallback();
    if (this.lastEnd === null) {
      this.armIdle();
      return;
    }
    this.afterFeed(scanner.feed(this.lastEnd, NO_BYTES));
  }

  private onTailIdle(token: number): void {
    this.idleTimer = null;
    if (token !== this.token || this.phase !== "scanning") return;
    const scanner = this.scanner as RepaintScanner;
    if (this.lastEnd === null) {
      if (this.fallback) this.holdAtCut(Infinity); // 아무것도 안 왔다: 바로 바꾼다
      return;
    }
    const cut = scanner.flushIdle();
    if (cut !== null) {
      this.holdAtCut(cut);
      return;
    }
    this.drain(scanner.safe);
    if (this.fallback) this.armIdle(); // CR 바로 뒤라 못 잘랐다
  }

  private armIdle(): void {
    this.clearIdle();
    const token = this.token;
    this.idleTimer = this.port.setTimer(() => this.onTailIdle(token), this.tailIdleMs);
  }

  private clearIdle(): void {
    if (this.idleTimer !== null) this.port.clearTimer(this.idleTimer);
    this.idleTimer = null;
  }

  private clearTimers(): void {
    this.clearIdle();
    if (this.deadlineTimer !== null) this.port.clearTimer(this.deadlineTimer);
    this.deadlineTimer = null;
  }

  // ---- cut ----

  private holdAtCut(cut: number): void {
    const step = this.steps[this.stepIndex];
    const token = this.token;
    this.clearTimers();
    this.scanner = null;
    this.phase = "holding";
    if (Number.isFinite(cut)) this.minSeq = step.cut === "A" ? cut + 3 : cut;
    this.drain(cut, () => this.completeStep(token));
  }

  /** cut 앞을 다 쓴 뒤: xterm 크기를 바꾸고, 붙잡은 프레임을 풀고, 다음 일로 넘어간다 */
  private completeStep(token: number): void {
    if (token !== this.token || this.disposed) return;
    this.port.applyLocal(this.steps[this.stepIndex].to);
    this.drain(Infinity);
    this.phase = "idle";
    this.stepIndex++;
    if (this.desired !== null) this.beginPlan();
    else if (this.stepIndex < this.steps.length) this.sendStep();
  }

  /** 큐 앞에서 limit(절대 seq) 아래만 순서대로 쓴다. 마지막 조각의 콜백에서 after 를 부른다 */
  private drain(limit: number, after?: () => void): void {
    const pieces: Held[] = [];
    while (this.queue.length > 0) {
      const head = this.queue[0];
      const end = head.seq + head.bytes.length;
      if (head.seq >= limit) break;
      if (end <= limit) {
        pieces.push(head);
        this.queue.shift();
        continue;
      }
      const n = limit - head.seq;
      pieces.push({ seq: head.seq, bytes: head.bytes.subarray(0, n), done: noop });
      this.queue[0] = { seq: limit, bytes: head.bytes.subarray(n), done: head.done };
      break;
    }
    pieces.forEach((piece, index) => {
      const isLast = index === pieces.length - 1;
      if (!(isLast && after)) {
        this.port.write(piece.bytes, piece.done);
        return;
      }
      this.port.write(piece.bytes, () => {
        piece.done();
        after();
      });
    });
    if (pieces.length === 0) after?.();
  }
}
