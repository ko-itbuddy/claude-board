//! 자율주행 실행 읽기 — 보드가 1초마다 실행 폴더(`runs\<id>\`)를 훑어 스냅샷의 `autopilot` 목록을 만들고,
//! 사람이 누른 일시정지·재개·중단을 `control.json` 으로 남긴다.
//!
//! 보드는 읽기와 `control.json` 쓰기만 한다. 실행 상태의 주인은 엔진이다.
//! 읽는 파일은 `run.json`·`queue.json`·지금 단위의 `units\<n>\engine.json` 셋뿐이다(`events.jsonl`·`stream.jsonl` 은 크고 화면에 없다).
//! 파일마다 `(수정 시각, 길이)` 가 같으면 다시 읽지 않고, 엔진이 쓰는 중이라 JSON 이 깨져 있으면 앞 값을 그대로 둔다.
//!
//! 엔진이 살아 있는지는 보지 않는다. 대기·일시정지 동안 엔진은 run.json 을 오래 쓰지 않으므로 `updated` 로는
//! 죽음과 대기를 가르지 못한다(작업 스케줄러가 1분 안에 다시 띄운다).

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use claude_board_autopilot::cli::valid_run_id;
use claude_board_autopilot::model::{from_slice, to_pretty_json, ControlFile, ControlRequest, Num, Ordered, QueueFile, QueueUnit, Run, RunFile, Slot, UnitProgress, UnitProgressFile};
use claude_board_autopilot::rules::control::{board_can_control, run_ended};
use claude_board_core::envs::{EnvChoice, EnvEntry, EnvKind};
use claude_board_core::{clock, fileio};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::State;

use crate::board::Board;
use crate::util::{board_log, ms};

type B<'a> = State<'a, Arc<Board>>;

/// 끝난 실행을 목록에 두는 시간(초). 이보다 오래 지나면 뺀다(파일은 그대로).
const ENDED_KEEP_SEC: f64 = 600.0;
/// 보드가 쓸 수 있는 요청 셋 — 순서가 `can` 의 순서다.
const BOARD_ACTIONS: [&str; 3] = ["pause", "resume", "stop"];

/// 이 상태에서 이 요청이 뜻이 있는가 — 규칙(`board_can_control`)은 일시정지 중의 일시정지와 도는 중의 재개도 받아 주므로
/// (엔진이 "이미 일시정지"·"일시정지 상태가 아니라 그대로"라고 넘긴다) 단추로는 일시정지 중이 아닐 때 일시정지, 일시정지 중일 때만 재개를 낸다.
fn can_send(action: &str, state: &str) -> bool {
    let paused = state == "paused";
    board_can_control(action, state) && match action {
        "pause" => !paused,
        "resume" => paused,
        _ => true,
    }
}

// ── 뷰 (스냅샷 `autopilot`) ─────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotRunView {
    pub id: String,
    /// 카드 짝짓기용 경로 — run.json `project_root`, 없으면 첫 단위 cwd. Windows 경로 글 그대로
    pub path: String,
    pub title: String,
    /// run.json state 글 그대로
    pub state: String,
    pub reason: String,
    /// 대기 끝 시각(ms)
    pub wait_until: Option<i64>,
    pub env: AutopilotEnvView,
    pub unit: Option<AutopilotUnitView>,
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub capped: usize,
    pub skipped: usize,
    pub stopped: usize,
    /// pause·resume·stop 가운데 지금 상태에서 보드가 보낼 수 있는 것(`can_send`)
    pub can: Vec<String>,
    pub ended: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotEnvView {
    pub key: String,
    pub label: String,
    pub sandbox: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotUnitView {
    pub n: usize,
    pub title: String,
    pub phase: String,
    pub active_sec: f64,
    /// 이번 단위의 비용. 실행 전체 합계는 끝난 단위 파일을 다 읽어야 해서 싣지 않는다
    pub cost_usd: f64,
    pub tries: i64,
}

// ── 파일 캐시 ──────────────────────────────────────

/// 파일이 바뀌었는지 보는 표지 — 수정 시각과 길이
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

fn stamp_of(path: &Path) -> Option<Stamp> {
    let meta = fs::metadata(path).ok()?;
    Some(Stamp { modified: meta.modified().ok(), len: meta.len() })
}

fn unix_secs(time: Option<SystemTime>) -> f64 {
    time.and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map_or(0.0, |d| d.as_secs_f64())
}

/// 파일 하나의 읽은 값. 표지가 같으면 다시 읽지 않는다.
#[derive(Debug)]
struct Cached<T> {
    stamp: Option<Stamp>,
    value: Option<T>,
}

impl<T> Default for Cached<T> {
    fn default() -> Self {
        Self { stamp: None, value: None }
    }
}

impl<T: DeserializeOwned> Cached<T> {
    /// 파일을 본다. 없으면 값을 비우고, 바뀌었으면 읽는다(`reads` 를 하나 센다).
    /// 읽다가 JSON 이 깨졌으면 앞 값과 앞 표지를 둔다 — 다음 틱에 다시 읽는다.
    fn refresh(&mut self, path: &Path, reads: &mut usize) {
        let Some(now_stamp) = stamp_of(path) else {
            *self = Self::default();
            return;
        };
        if self.stamp == Some(now_stamp) {
            return;
        }
        *reads += 1;
        if let Some(value) = fs::read(path).ok().and_then(|bytes| from_slice::<T>(&bytes).ok()) {
            self.value = Some(value);
            self.stamp = Some(now_stamp);
        }
    }
}

/// 실행 폴더 하나의 캐시
#[derive(Default)]
struct RunCache {
    run: Cached<RunFile>,
    queue: Cached<QueueFile>,
    /// (단위 번호, engine.json). 단위가 바뀌면 새로 읽는다
    unit: (u32, Cached<UnitProgressFile>),
}

impl RunCache {
    fn run_state(&self) -> &str {
        self.run.value.as_ref().and_then(|r| r.state.get()).map_or("", String::as_str)
    }

    /// 목록에서 뺄 만큼 오래전에 끝났는가
    fn retired(&self, now: f64) -> bool {
        let Some(run) = self.run.value.as_ref() else { return false };
        if !run_ended(self.run_state()) {
            return false;
        }
        let modified = unix_secs(self.run.stamp.and_then(|s| s.modified));
        let ended = run.ended.get().and_then(Num::as_f64).unwrap_or(modified);
        now - ended > ENDED_KEEP_SEC
    }

    fn current_unit(&self) -> Option<u32> {
        self.run.value.as_ref()?.unit.get().copied().filter(|n| *n > 0)
    }

    fn refresh(&mut self, dir: &Path, now: f64, reads: &mut usize) {
        let run_path = dir.join("run.json");
        let before = self.run.stamp;
        self.run.refresh(&run_path, reads);
        // 오래전에 끝난 실행은 run.json 이 그대로인 동안 다른 파일을 보지 않는다
        if self.retired(now) && before == self.run.stamp {
            return;
        }
        self.queue.refresh(&dir.join("queue.json"), reads);
        match self.current_unit() {
            Some(n) => {
                if self.unit.0 != n {
                    self.unit = (n, Cached::default());
                }
                self.unit.1.refresh(&dir.join("units").join(n.to_string()).join("engine.json"), reads);
            }
            None => self.unit = (0, Cached::default()),
        }
    }
}

// ── 읽기 ──────────────────────────────────────────

pub struct AutopilotReader {
    runs_dir: PathBuf,
    caches: HashMap<String, RunCache>,
    /// 파일을 실제로 읽어 해석한 횟수 — 캐시 시험이 센다
    reads: usize,
}

impl AutopilotReader {
    /// `root` 는 `claude_board_core::paths::autopilot_root(false)`. 실행 폴더는 그 아래 `runs\<id>\`
    pub fn new(root: PathBuf) -> Self {
        Self { runs_dir: root.join("runs"), caches: HashMap::new(), reads: 0 }
    }

    /// 1초마다. `runs\` 아래 폴더를 훑고, 파일이 바뀐 것만 다시 읽는다. `now` 는 유닉스 초.
    pub fn refresh(&mut self, now: f64) {
        let ids = run_folder_ids(&self.runs_dir);
        self.caches.retain(|id, _| ids.contains(id));
        for id in ids {
            let dir = self.runs_dir.join(&id);
            self.caches.entry(id).or_default().refresh(&dir, now, &mut self.reads);
        }
    }

    /// 목록에 보일 실행들 — 새 실행이 앞에 온다.
    pub fn view(&self, now: f64) -> Vec<AutopilotRunView> {
        let mut shown: Vec<(f64, AutopilotRunView)> = self
            .caches
            .iter()
            .filter(|(_, cache)| !cache.run_state().is_empty() && !cache.retired(now))
            .filter_map(|(id, cache)| {
                let created = cache.run.value.as_ref()?.created.get().and_then(Num::as_f64).unwrap_or(0.0);
                Some((created, run_view(id, cache)?))
            })
            .collect();
        shown.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
        shown.into_iter().map(|(_, view)| view).collect()
    }

    /// 읽은 실행의 폴더. 캐시에 있는 id 만 답한다(경로 조각이 섞인 id 를 걸러낸다).
    pub fn run_dir(&self, run_id: &str) -> Option<PathBuf> {
        self.caches.contains_key(run_id).then(|| self.runs_dir.join(run_id))
    }

    /// 지금 run.json 의 state. 아직 출발 준비 중이면(state 가 비면) None
    pub fn state_of(&self, run_id: &str) -> Option<String> {
        let state = self.caches.get(run_id)?.run_state();
        (!state.is_empty()).then(|| state.to_string())
    }

    /// 요청을 써도 되는지 본다 — 아는 요청이고 지금 상태에서 보낼 수 있으면 `control.json` 을 둘 실행 폴더.
    pub fn control_target(&self, run_id: &str, action: &str) -> Result<PathBuf, String> {
        if !BOARD_ACTIONS.contains(&action) {
            return Err("모르는 요청".to_string());
        }
        let state = self.state_of(run_id).ok_or("모르는 실행")?;
        if !can_send(action, &state) {
            return Err("지금 상태에서는 할 수 없습니다".to_string());
        }
        self.run_dir(run_id).ok_or_else(|| "모르는 실행".to_string())
    }
}

/// `runs\` 아래 폴더 이름들. 폴더가 없으면 빈 목록
fn run_folder_ids(runs_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(runs_dir) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}

// ── 뷰 만들기 (순수) ────────────────────────────────

fn count_status(queue: &[Ordered<QueueUnit>], status: &str) -> usize {
    queue.iter().filter(|u| u.status.get().is_some_and(|s| s == status)).count()
}

fn text(slot: &Slot<String>) -> String {
    slot.get().cloned().unwrap_or_default()
}

/// 실행 환경 이름 — WSL 은 "WSL · 배포판". 키를 못 알아보면 kind, 그것도 못 알아보면 키 그대로
fn env_label(key: &str, kind: &str) -> String {
    match EnvChoice::parse(key) {
        Some(EnvChoice { kind: EnvKind::Wsl, distro: Some(distro) }) => format!("{} · {distro}", EnvKind::Wsl.label()),
        Some(choice) => choice.kind.label().to_string(),
        None => EnvKind::from_id(kind).map_or_else(|| key.to_string(), |k| k.label().to_string()),
    }
}

fn env_view(run: &Run) -> AutopilotEnvView {
    match run.exec_env.get() {
        Some(env) => AutopilotEnvView { key: env.key.clone(), label: env_label(&env.key, &env.kind), sandbox: env.sandbox.unwrap_or(false) },
        None => AutopilotEnvView { key: String::new(), label: "알 수 없음".to_string(), sandbox: false },
    }
}

fn unit_view(cache: &RunCache, queue: &[Ordered<QueueUnit>]) -> Option<AutopilotUnitView> {
    let n = cache.current_unit()?;
    let progress = cache.unit.1.value.as_ref();
    let number = |pick: fn(&UnitProgress) -> &Slot<Num>| -> f64 {
        progress.and_then(|p| pick(p).get()).and_then(Num::as_f64).unwrap_or(0.0)
    };
    let title = progress
        .map(|p| text(&p.title))
        .filter(|t| !t.is_empty())
        .or_else(|| queue.get(n as usize - 1).map(|u| text(&u.title)))
        .unwrap_or_default();
    Some(AutopilotUnitView {
        n: n as usize,
        title,
        phase: progress.map(|p| text(&p.phase)).unwrap_or_default(),
        active_sec: number(|p| &p.active_sec),
        cost_usd: number(|p| &p.cost_usd),
        tries: number(|p| &p.r#try) as i64,
    })
}

/// 카드 짝짓기용 경로 — project_root, 없으면 첫 단위 cwd
fn run_path(run: &Run, queue: &[Ordered<QueueUnit>]) -> String {
    Some(text(&run.project_root))
        .filter(|p| !p.is_empty())
        .or_else(|| queue.first().map(|u| text(&u.cwd)).filter(|p| !p.is_empty()))
        .unwrap_or_default()
}

fn run_view(id: &str, cache: &RunCache) -> Option<AutopilotRunView> {
    let run = cache.run.value.as_ref()?;
    let queue: &[Ordered<QueueUnit>] = cache.queue.value.as_deref().unwrap_or(&[]);
    let state = text(&run.state);
    let title = Some(text(&run.title)).filter(|t| !t.is_empty()).or_else(|| Some(text(&run.project)).filter(|p| !p.is_empty())).unwrap_or_else(|| id.to_string());
    Some(AutopilotRunView {
        id: id.to_string(),
        path: run_path(run, queue),
        title,
        reason: text(&run.reason),
        wait_until: run.wait_until.get().and_then(Num::as_f64).map(ms),
        env: env_view(run),
        unit: unit_view(cache, queue),
        total: queue.len(),
        done: count_status(queue, "done"),
        failed: count_status(queue, "failed"),
        capped: count_status(queue, "capped"),
        skipped: count_status(queue, "skipped"),
        stopped: count_status(queue, "stopped"),
        can: BOARD_ACTIONS.iter().filter(|a| can_send(a, &state)).map(|a| a.to_string()).collect(),
        ended: run_ended(&state),
        state,
    })
}

// ── control.json 쓰기 ──────────────────────────────

/// `{"action", "ts"}` 를 실행 폴더의 `control.json` 으로 쓴다. 엔진이 1초마다 보다가 받으면 지운다.
fn write_control(run_dir: &Path, action: &str, now: f64) -> Result<(), String> {
    let request = ControlRequest { action: action.to_string().into(), ts: Num::r3(now).into(), ..Default::default() };
    let body = to_pretty_json(&ControlFile::new(request)).map_err(|e| format!("요청을 만들지 못했습니다: {e}"))?;
    fileio::write_atomic(&run_dir.join("control.json"), "control", body.as_bytes()).map_err(|e| format!("요청을 쓰지 못했습니다: {e}"))
}

/// 자율주행 실행에 일시정지·재개·중단을 보낸다. 중단은 확인 창을 먼저 띄운다(취소면 아무것도 쓰지 않고 끝난다).
#[tauri::command(async)]
pub fn autopilot_control(board: B<'_>, run_id: String, action: String) -> Result<(), String> {
    // 잠금은 이 문장에서 풀린다 — 확인 창이 떠 있는 동안 1초 틱이 멈추지 않게
    let run_dir = board.autopilot.lock().unwrap().control_target(&run_id, &action)?;
    if action == "stop" && !board.confirm("자율주행 중단", "이 실행을 중단할까요? 지금 단위는 중단으로 기록되고 다시 띄우지 않습니다.", "중단", "취소") {
        return Ok(());
    }
    write_control(&run_dir, &action, clock::now())?;
    board_log(&format!("[자율주행] {run_id} {action}"));
    board.publish();
    Ok(())
}

// ── 시작 준비 ───────────────────────────────────────
//
// 시작 준비 화면이 보낸 출발 요청을 실행 폴더(`runs\<id>\run.json`·`queue.json`)로 쓰고 `claude-autopilot start <폴더> --json` 을 부른다.
// 출발 규칙(실행 환경 문턱·경로 확인·판정 명령 꼴·가지·작업 스케줄러 등록)은 명령줄이 소유한다. 여기서는 규칙을 다시 적지 않고,
// 파일을 쓰기 전에 입력 모양(실행 id·크기·동의 기록이 끼어들지 않았는지)만 보고, 명령줄이 거절하면 그 글을 그대로 돌려준다.

/// 명령줄 실행 파일 이름(`claude-autopilot` 크레이트의 `[[bin]]`)
const CLI_EXE: &str = if cfg!(windows) { "claude-autopilot.exe" } else { "claude-autopilot" };
/// 한 번에 담을 수 있는 단위 수와 요청 크기(바이트). 화면의 `MAX_UNITS` 와 같은 값
const MAX_UNITS: usize = 30;
const MAX_REQUEST_BYTES: usize = 1 << 20;
/// 출발 명령이 끝나기를 기다리는 시간. WSL 준비 확인이 배포판을 깨우므로 넉넉히 둔다
const START_TIMEOUT: Duration = Duration::from_secs(300);

/// 화면이 고를 수 있는 실행 환경 한 개
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutopilotEnvOption {
    pub key: String,
    pub label: String,
    pub kind: String,
    pub distro: Option<String>,
    /// Claude Code 샌드박스를 걸 수 있는 환경(WSL)이면 참. 거짓이면 화면이 샌드박스 없음 동의를 받는다
    pub sandbox_capable: bool,
}

fn env_option(entry: &EnvEntry) -> AutopilotEnvOption {
    AutopilotEnvOption {
        key: entry.choice.key(),
        label: entry.label.clone(),
        kind: entry.choice.kind.id().to_string(),
        distro: entry.choice.distro.clone(),
        sandbox_capable: entry.choice.kind.sandbox_capable(),
    }
}

/// 이 PC 에서 자율주행 실행 환경으로 고를 수 있는 목록. 기본값은 없다(고르는 것은 사람)
#[tauri::command(async)]
pub fn autopilot_envs() -> Vec<AutopilotEnvOption> {
    claude_board_core::envs::detect().iter().map(env_option).collect()
}

/// 화면이 보낸 출발 요청
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartRequest {
    run_id: String,
    run: Value,
    queue: Vec<Value>,
    accept_unsandboxed: bool,
}

/// 출발한 결과
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDone {
    run_id: String,
    scheduled: bool,
}

/// 모델 이름 꼴(`claude --model` 에 넘기는 값). 명령줄이 보지 않으므로 여기서 본다
fn valid_model(model: &str) -> bool {
    let mut chars = model.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '[' | ']' | '-'))
}

/// 입력 모양 검사. 경로·환경·판정 명령은 보지 않는다(명령줄이 본다)
fn validate_request(req: &StartRequest) -> Result<(), String> {
    if !valid_run_id(&req.run_id) {
        return Err(format!("실행 id 규칙(영문·숫자·점·밑줄·붙임표)에 맞지 않습니다: {}", req.run_id));
    }
    let run = req.run.as_object().ok_or("run.json 내용이 객체가 아닙니다.")?;
    if req.queue.is_empty() || req.queue.len() > MAX_UNITS {
        return Err(format!("단위는 1개 이상 {MAX_UNITS}개 이하여야 합니다."));
    }
    if req.queue.iter().any(|u| !u.is_object()) {
        return Err("queue.json 의 단위가 객체가 아닙니다.".to_string());
    }
    if serde_json::to_vec(&req.run).map_or(0, |b| b.len()) + serde_json::to_vec(&req.queue).map_or(0, |b| b.len()) > MAX_REQUEST_BYTES {
        return Err("요청이 너무 큽니다.".to_string());
    }
    if !run.get("model").and_then(Value::as_str).is_some_and(valid_model) {
        return Err("모델 이름이 비었거나 꼴이 아닙니다.".to_string());
    }
    // 샌드박스 없음 동의는 `--accept-unsandboxed` 로만 남긴다. 요청 안에 동의 기록이 있으면 그 단추를 건너뛰는 길이 된다
    if run.get("exec_env").and_then(|e| e.get("unsandboxed_ok")).is_some() {
        return Err("동의 기록(unsandboxed_ok)은 요청에 담을 수 없습니다. 동의는 화면의 확인 칸으로만 받습니다.".to_string());
    }
    Ok(())
}

/// `claude-autopilot start <실행 폴더> --json [--accept-unsandboxed]`
fn start_args(dir: &Path, accept_unsandboxed: bool) -> Vec<String> {
    let mut args = vec!["start".to_string(), dir.to_string_lossy().into_owned(), "--json".to_string()];
    if accept_unsandboxed {
        args.push("--accept-unsandboxed".to_string());
    }
    args
}

/// 실행 폴더를 새로 만들고 `run.json`·`queue.json` 을 쓴다. 같은 이름의 폴더가 이미 있으면 거부한다(남의 실행을 덮지 않는다)
fn write_run_folder(runs_dir: &Path, req: &StartRequest) -> Result<PathBuf, String> {
    fs::create_dir_all(runs_dir).map_err(|e| format!("실행 폴더의 부모({})를 만들지 못했습니다: {e}", runs_dir.display()))?;
    let dir = runs_dir.join(&req.run_id);
    fs::create_dir(&dir).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            format!("같은 이름의 실행 폴더가 이미 있습니다: {}", dir.display())
        } else {
            format!("실행 폴더를 만들지 못했습니다({}): {e}", dir.display())
        }
    })?;
    let written = write_json_file(&dir.join("queue.json"), &req.queue).and_then(|()| write_json_file(&dir.join("run.json"), &req.run));
    if let Err(why) = written {
        discard_run_folder(&dir);
        return Err(why);
    }
    Ok(dir)
}

fn write_json_file<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), String> {
    let mut body = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    body.push('\n');
    fileio::write_atomic(path, "start", body.as_bytes()).map_err(|e| format!("{} 을(를) 쓰지 못했습니다: {e}", path.display()))
}

/// 출발하지 못한 실행 폴더를 치운다. 우리가 쓴 두 파일만 지우고 폴더가 비어 있을 때만 폴더를 지운다
fn discard_run_folder(dir: &Path) {
    for name in ["run.json", "queue.json"] {
        let _ = fs::remove_file(dir.join(name));
    }
    let _ = fs::remove_dir(dir);
}

struct CliOutput {
    code: i32,
    stdout: String,
    stderr: String,
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_end(&mut buf);
        }
        buf
    })
}

/// 명령줄을 콘솔 창 없이 돌리고 끝나기를 기다린다. 시간이 지나면 죽이고 실패로 돌려준다
fn run_cli(exe: &Path, args: &[String], timeout: Duration) -> Result<CliOutput, String> {
    let mut cmd = Command::new(exe);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(crate::util::CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| format!("{} 를 띄우지 못했습니다: {e}", exe.display()))?;
    let (out, err) = (drain(child.stdout.take()), drain(child.stderr.take()));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("출발 명령이 {}초 안에 끝나지 않아 멈췄습니다.", timeout.as_secs()));
            }
            Err(e) => return Err(format!("출발 명령을 기다리지 못했습니다: {e}")),
        }
    };
    let text = |h: JoinHandle<Vec<u8>>| String::from_utf8_lossy(&h.join().unwrap_or_default()).into_owned();
    Ok(CliOutput { code: status.code().unwrap_or(-1), stdout: text(out), stderr: text(err) })
}

/// 명령줄이 거절한 까닭 글 — `--json` 의 `problems` 를 줄마다, 없으면 표준 오류, 그것도 없으면 종료 코드
fn failure_text(out: &CliOutput) -> String {
    let problems: Vec<String> = serde_json::from_str::<Value>(out.stdout.trim())
        .ok()
        .and_then(|v| v.get("problems").and_then(Value::as_array).cloned())
        .map(|a| a.iter().filter_map(|p| p.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    if !problems.is_empty() {
        return problems.join("\n");
    }
    let err = out.stderr.trim();
    if !err.is_empty() {
        return err.to_string();
    }
    format!("출발 명령이 실패했습니다(종료 코드 {}).", out.code)
}

/// 출발의 몸통 — 실행 폴더를 쓰고 명령줄을 부른다. 명령줄이 거절하면 쓴 폴더를 치우고 그 글을 돌려준다
fn start_run(exe: &Path, runs_dir: &Path, req: &StartRequest, timeout: Duration) -> Result<StartDone, String> {
    validate_request(req)?;
    let dir = write_run_folder(runs_dir, req)?;
    let out = match run_cli(exe, &start_args(&dir, req.accept_unsandboxed), timeout) {
        Ok(out) => out,
        Err(why) => {
            discard_run_folder(&dir);
            return Err(why);
        }
    };
    if out.code != 0 {
        discard_run_folder(&dir);
        return Err(failure_text(&out));
    }
    let scheduled = serde_json::from_str::<Value>(out.stdout.trim()).ok().and_then(|v| v.get("scheduled").and_then(Value::as_bool)).unwrap_or(false);
    Ok(StartDone { run_id: req.run_id.clone(), scheduled })
}

/// 명령줄 실행 파일 — 보드 실행 파일과 같은 폴더(엔진 `claude-autopilot-engine` 을 찾는 방식과 같다)
fn cli_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("보드 실행 파일 위치를 알 수 없습니다: {e}"))?.with_file_name(CLI_EXE);
    if exe.is_file() {
        Ok(exe)
    } else {
        Err(format!("{CLI_EXE} 를 찾지 못했습니다: {}. 보드 실행 파일과 같은 폴더에 있어야 합니다.", exe.display()))
    }
}

/// 자율주행을 시작한다. 실패하면 명령줄의 거절 글(여러 줄일 수 있다)이 그대로 `Err` 로 간다
#[tauri::command(async)]
pub fn autopilot_start(board: B<'_>, request: StartRequest) -> Result<StartDone, String> {
    let exe = cli_exe()?;
    let runs_dir = claude_board_core::paths::autopilot_root(false).join("runs");
    let done = start_run(&exe, &runs_dir, &request, START_TIMEOUT).inspect_err(|why| board_log(&format!("[자율주행] 출발 실패 {}: {}", request.run_id, why.replace('\n', " / "))))?;
    board_log(&format!("[자율주행] 출발 {}", done.run_id));
    board.publish();
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    const NOW: f64 = 2_000_000_000.0;

    struct Sandbox {
        root: PathBuf,
    }

    impl Sandbox {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("cb-autopilot-{}", crate::util::short_id()));
            fs::create_dir_all(root.join("runs")).unwrap();
            Self { root }
        }

        fn run_dir(&self, id: &str) -> PathBuf {
            self.root.join("runs").join(id)
        }

        fn put(&self, id: &str, rel: &str, value: &Value) {
            let path = self.run_dir(id).join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, value.to_string()).unwrap();
        }

        fn reader(&self) -> AutopilotReader {
            AutopilotReader::new(self.root.clone())
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn running_run() -> Value {
        json!({"state": "running", "title": "야간 작업", "project_root": r"C:\work\app", "unit": 2, "created": NOW - 100.0,
               "exec_env": {"key": "pwsh", "kind": "pwsh", "sandbox": false}})
    }

    fn queue_of(statuses: &[&str]) -> Value {
        Value::Array(statuses.iter().enumerate().map(|(i, s)| json!({"id": format!("u{i}"), "title": format!("단위 {}", i + 1), "cwd": r"C:\work\unit", "status": s})).collect())
    }

    fn view_of(sandbox: &Sandbox, id: &str) -> AutopilotRunView {
        let mut reader = sandbox.reader();
        reader.refresh(NOW);
        reader.view(NOW).into_iter().find(|v| v.id == id).expect("목록에 있어야 한다")
    }

    #[test]
    fn reads_running_run() {
        let sandbox = Sandbox::new();
        sandbox.put("r1", "run.json", &json!({"state": "waiting_limit", "title": "야간 작업", "reason": "한도", "wait_until": 1_999_999_999.5, "created": NOW - 5.0}));
        let view = view_of(&sandbox, "r1");
        assert_eq!((view.title.as_str(), view.state.as_str(), view.reason.as_str()), ("야간 작업", "waiting_limit", "한도"));
        assert_eq!(view.wait_until, Some(1_999_999_999_500));
        assert!(!view.ended);
        assert_eq!(view.unit, None);
    }

    #[test]
    fn path_from_project_root_then_first_cwd() {
        let sandbox = Sandbox::new();
        sandbox.put("a", "run.json", &running_run());
        sandbox.put("a", "queue.json", &queue_of(&["running"]));
        assert_eq!(view_of(&sandbox, "a").path, r"C:\work\app");
        sandbox.put("b", "run.json", &json!({"state": "running"}));
        sandbox.put("b", "queue.json", &queue_of(&["running"]));
        assert_eq!(view_of(&sandbox, "b").path, r"C:\work\unit");
    }

    #[test]
    fn counts_from_queue() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &running_run());
        sandbox.put("r", "queue.json", &queue_of(&["done", "done", "failed", "capped", "skipped", "stopped", "running", "pending"]));
        let v = view_of(&sandbox, "r");
        assert_eq!((v.total, v.done, v.failed, v.capped, v.skipped, v.stopped), (8, 2, 1, 1, 1, 1));
    }

    #[test]
    fn current_unit_from_engine_json() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &running_run());
        sandbox.put("r", "queue.json", &queue_of(&["done", "running"]));
        sandbox.put("r", "units/2/engine.json", &json!({"title": "엔진이 쓴 제목", "phase": "model", "active_sec": 12.5, "cost_usd": 0.42, "try": 1}));
        let unit = view_of(&sandbox, "r").unit.expect("지금 단위");
        assert_eq!((unit.n, unit.title.as_str(), unit.phase.as_str(), unit.tries), (2, "엔진이 쓴 제목", "model", 1));
        assert_eq!((unit.active_sec, unit.cost_usd), (12.5, 0.42));
    }

    #[test]
    fn current_unit_title_falls_back_to_queue() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &running_run());
        sandbox.put("r", "queue.json", &queue_of(&["done", "running"]));
        let unit = view_of(&sandbox, "r").unit.expect("지금 단위");
        assert_eq!((unit.title.as_str(), unit.phase.as_str(), unit.cost_usd), ("단위 2", "", 0.0));
    }

    #[test]
    fn skips_preparing_run() {
        let sandbox = Sandbox::new();
        fs::create_dir_all(sandbox.run_dir("empty")).unwrap();
        sandbox.put("blank", "run.json", &json!({"state": ""}));
        sandbox.put("none", "run.json", &json!({"title": "아직"}));
        let mut reader = sandbox.reader();
        reader.refresh(NOW);
        assert!(reader.view(NOW).is_empty());
        assert_eq!(reader.state_of("blank"), None);
    }

    #[test]
    fn drops_ended_after_600s() {
        let sandbox = Sandbox::new();
        sandbox.put("old", "run.json", &json!({"state": "done", "ended": NOW - 601.0}));
        let mut reader = sandbox.reader();
        reader.refresh(NOW);
        assert!(reader.view(NOW).is_empty());
        // 뺀 뒤에는 run.json 이 그대로인 동안 다른 파일을 다시 읽지 않는다
        let reads = reader.reads;
        reader.refresh(NOW + 5.0);
        assert_eq!(reader.reads, reads);
    }

    #[test]
    fn keeps_ended_within_600s() {
        let sandbox = Sandbox::new();
        sandbox.put("fresh", "run.json", &json!({"state": "stopped", "ended": NOW - 599.0}));
        let v = view_of(&sandbox, "fresh");
        assert!(v.ended);
        assert!(v.can.is_empty());
    }

    #[test]
    fn mtime_cache_skips_unchanged() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &running_run());
        sandbox.put("r", "queue.json", &queue_of(&["running", "running"]));
        sandbox.put("r", "units/2/engine.json", &json!({"phase": "action"}));
        let mut reader = sandbox.reader();
        reader.refresh(NOW);
        assert_eq!(reader.reads, 3);
        reader.refresh(NOW + 1.0);
        reader.refresh(NOW + 2.0);
        assert_eq!(reader.reads, 3, "바뀐 파일이 없으면 다시 읽지 않는다");
        // 길이가 달라지면 시각이 같아도 다시 읽는다
        sandbox.put("r", "units/2/engine.json", &json!({"phase": "check"}));
        reader.refresh(NOW + 3.0);
        assert_eq!(reader.reads, 4);
    }

    #[test]
    fn broken_json_keeps_previous() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &running_run());
        let mut reader = sandbox.reader();
        reader.refresh(NOW);
        fs::write(sandbox.run_dir("r").join("run.json"), b"{\"state\": \"runn").unwrap();
        reader.refresh(NOW + 1.0);
        let views = reader.view(NOW + 1.0);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].state, "running");
    }

    #[test]
    fn removed_folder_drops() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &running_run());
        let mut reader = sandbox.reader();
        reader.refresh(NOW);
        assert_eq!(reader.view(NOW).len(), 1);
        fs::remove_dir_all(sandbox.run_dir("r")).unwrap();
        reader.refresh(NOW + 1.0);
        assert!(reader.view(NOW + 1.0).is_empty());
        assert_eq!(reader.run_dir("r"), None);
    }

    #[test]
    fn env_view_native_and_wsl() {
        let sandbox = Sandbox::new();
        sandbox.put("n", "run.json", &running_run());
        sandbox.put("w", "run.json", &json!({"state": "running", "exec_env": {"key": "wsl:Ubuntu-24.04", "kind": "wsl", "distro": "Ubuntu-24.04", "sandbox": true}}));
        let native = view_of(&sandbox, "n").env;
        assert_eq!((native.key.as_str(), native.label.as_str(), native.sandbox), ("pwsh", "PowerShell 7", false));
        let wsl = view_of(&sandbox, "w").env;
        assert_eq!((wsl.key.as_str(), wsl.label.as_str(), wsl.sandbox), ("wsl:Ubuntu-24.04", "WSL · Ubuntu-24.04", true));
    }

    #[test]
    fn env_view_missing() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &json!({"state": "running"}));
        let env = view_of(&sandbox, "r").env;
        assert_eq!((env.key.as_str(), env.label.as_str(), env.sandbox), ("", "알 수 없음", false));
    }

    #[test]
    fn can_by_state() {
        let sandbox = Sandbox::new();
        for (id, state) in [("a", "running"), ("b", "paused"), ("c", "done")] {
            sandbox.put(id, "run.json", &json!({"state": state, "ended": NOW}));
        }
        assert_eq!(view_of(&sandbox, "a").can, ["pause", "stop"]);
        assert_eq!(view_of(&sandbox, "b").can, ["resume", "stop"]);
        assert!(view_of(&sandbox, "c").can.is_empty());
    }

    #[test]
    fn control_writes_file() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &running_run());
        let mut reader = sandbox.reader();
        reader.refresh(NOW);
        let dir = reader.control_target("r", "pause").unwrap();
        write_control(&dir, "pause", 1_761_000_000.123_456).unwrap();
        let written: Value = serde_json::from_slice(&fs::read(sandbox.run_dir("r").join("control.json")).unwrap()).unwrap();
        assert_eq!(written, json!({"action": "pause", "ts": 1_761_000_000.123}));
    }

    #[test]
    fn control_refuses_unknown_action() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &running_run());
        let mut reader = sandbox.reader();
        reader.refresh(NOW);
        assert_eq!(reader.control_target("r", "takeover"), Err("모르는 요청".to_string()));
        assert!(!sandbox.run_dir("r").join("control.json").exists());
    }

    #[test]
    fn control_refuses_by_state() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &json!({"state": "paused"}));
        sandbox.put("d", "run.json", &json!({"state": "done", "ended": NOW}));
        let mut reader = sandbox.reader();
        reader.refresh(NOW);
        let refused = Err("지금 상태에서는 할 수 없습니다".to_string());
        assert_eq!(reader.control_target("r", "pause"), refused);
        assert_eq!(reader.control_target("d", "stop"), refused);
        assert!(reader.control_target("r", "resume").is_ok());
    }

    #[test]
    fn run_dir_refuses_path_pieces() {
        let sandbox = Sandbox::new();
        sandbox.put("r", "run.json", &running_run());
        let mut reader = sandbox.reader();
        reader.refresh(NOW);
        assert_eq!(reader.run_dir(r"..\r"), None);
        assert!(reader.run_dir("r").is_some());
    }
}

#[cfg(test)]
mod start_tests {
    use super::*;
    use claude_board_core::envs::EnvChoice;
    use serde_json::json;

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!("cb-ap-start-{}", crate::util::short_id()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn runs(&self) -> PathBuf {
            self.0.join("runs")
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn request() -> StartRequest {
        StartRequest {
            run_id: "20261106-090503-toy".to_string(),
            run: json!({"exec_env": {"key": "pwsh", "kind": "pwsh"}, "model": "claude-sonnet-5-5", "project": "toy", "project_root": r"C:\work\toy", "title": "toy"}),
            queue: vec![json!({"check": "exit 0", "cwd": r"C:\work\toy", "id": "u1", "prompt": "p", "title": "t"})],
            accept_unsandboxed: true,
        }
    }

    fn refused(req: &StartRequest, needle: &str) {
        let why = validate_request(req).expect_err("거절해야 한다");
        assert!(why.contains(needle), "{needle:?} 가 없다: {why}");
    }

    #[test]
    fn model_shape() {
        for ok in ["claude-sonnet-5-5", "sonnet", "opus", "sonnet[1m]", "a.b_c-1"] {
            assert!(valid_model(ok), "{ok}");
        }
        for bad in ["", "Sonnet", "-x", "a b", "a;b", "한글", "a/b"] {
            assert!(!valid_model(bad), "{bad}");
        }
    }

    #[test]
    fn accepts_a_well_formed_request() {
        assert_eq!(validate_request(&request()), Ok(()));
    }

    #[test]
    fn refuses_run_ids_that_could_leave_the_runs_folder() {
        for bad in ["", ".", "..", "a/b", r"a\b", "a b", "한글"] {
            refused(&StartRequest { run_id: bad.to_string(), ..request() }, "실행 id");
        }
    }

    #[test]
    fn refuses_bad_shapes_and_sizes() {
        refused(&StartRequest { run: json!([]), ..request() }, "객체가 아닙니다");
        refused(&StartRequest { queue: vec![], ..request() }, "단위는");
        refused(&StartRequest { queue: vec![json!({}); MAX_UNITS + 1], ..request() }, "단위는");
        refused(&StartRequest { queue: vec![json!("x")], ..request() }, "단위가 객체가 아닙니다");
        let huge = StartRequest { queue: vec![json!({"prompt": "x".repeat(MAX_REQUEST_BYTES)})], ..request() };
        refused(&huge, "너무 큽니다");
        let mut no_model = request();
        no_model.run["model"] = json!("Bad Model");
        refused(&no_model, "모델");
        no_model.run.as_object_mut().unwrap().remove("model");
        refused(&no_model, "모델");
    }

    #[test]
    fn consent_cannot_ride_inside_the_request() {
        let mut req = request();
        req.run["exec_env"]["unsandboxed_ok"] = json!({"at": 1, "by": "board"});
        refused(&req, "동의 기록");
    }

    #[test]
    fn start_args_are_exactly_the_cli_contract() {
        let dir = Path::new(r"C:\runs\r1");
        assert_eq!(start_args(dir, false), ["start", r"C:\runs\r1", "--json"]);
        assert_eq!(start_args(dir, true), ["start", r"C:\runs\r1", "--json", "--accept-unsandboxed"]);
        // 실행 환경 문턱의 주인이 명령줄이므로 --dev·--no-schedule 은 붙이지 않는다
        assert!(!start_args(dir, true).iter().any(|a| a == "--dev" || a == "--no-schedule"));
    }

    #[test]
    fn writes_both_files_and_refuses_an_existing_folder() {
        let t = Temp::new();
        let req = request();
        let dir = write_run_folder(&t.runs(), &req).unwrap();
        assert_eq!(dir, t.runs().join("20261106-090503-toy"));
        let run: Value = serde_json::from_slice(&fs::read(dir.join("run.json")).unwrap()).unwrap();
        let queue: Value = serde_json::from_slice(&fs::read(dir.join("queue.json")).unwrap()).unwrap();
        assert_eq!(run, req.run);
        assert_eq!(queue, Value::Array(req.queue.clone()));
        let again = write_run_folder(&t.runs(), &req).unwrap_err();
        assert!(again.contains("이미 있습니다"), "{again}");
        // 먼저 있던 폴더의 파일을 건드리지 않는다
        assert!(dir.join("run.json").is_file());
    }

    #[test]
    fn discard_removes_only_what_the_board_wrote() {
        let t = Temp::new();
        let dir = write_run_folder(&t.runs(), &request()).unwrap();
        discard_run_folder(&dir);
        assert!(!dir.exists());
        let kept = write_run_folder(&t.runs(), &StartRequest { run_id: "other".to_string(), ..request() }).unwrap();
        fs::write(kept.join("note.txt"), "keep").unwrap();
        discard_run_folder(&kept);
        assert!(kept.join("note.txt").is_file(), "다른 파일이 든 폴더는 지우지 않는다");
        assert!(!kept.join("run.json").exists());
    }

    #[test]
    fn failure_text_prefers_problems_then_stderr_then_code() {
        let out = |code, stdout: &str, stderr: &str| CliOutput { code, stdout: stdout.to_string(), stderr: stderr.to_string() };
        assert_eq!(failure_text(&out(1, "{\"ok\":false,\"problems\":[\"가\",\"나\"]}\n", "")), "가\n나");
        assert_eq!(failure_text(&out(2, "", "알 수 없는 명령입니다: x\n사용법\n")), "알 수 없는 명령입니다: x\n사용법");
        assert_eq!(failure_text(&out(3, "not json", "")), "출발 명령이 실패했습니다(종료 코드 3).");
        assert_eq!(failure_text(&out(1, "{\"ok\":false,\"problems\":[]}", "e")), "e");
    }

    #[test]
    fn env_option_marks_only_wsl_sandbox_capable() {
        let entry = |kind, distro: Option<&str>| EnvEntry { choice: EnvChoice { kind, distro: distro.map(str::to_string) }, label: "x".into(), program: "x.exe".into() };
        let wsl = env_option(&entry(EnvKind::Wsl, Some("Ubuntu")));
        assert_eq!((wsl.key.as_str(), wsl.kind.as_str(), wsl.distro.as_deref(), wsl.sandbox_capable), ("wsl:Ubuntu", "wsl", Some("Ubuntu"), true));
        for kind in [EnvKind::Pwsh, EnvKind::PowerShell, EnvKind::Cmd, EnvKind::GitBash] {
            let e = env_option(&entry(kind, None));
            assert_eq!((e.key.as_str(), e.distro, e.sandbox_capable), (kind.id(), None, false));
        }
    }

    /// 가짜 명령줄(배치 파일) — 받은 인자를 옆 파일에 적고 정해 둔 글과 종료 코드로 끝난다
    #[cfg(windows)]
    fn fake_cli(t: &Temp, stdout: &str, code: i32) -> PathBuf {
        let path = t.0.join("fake-cli.cmd");
        let body = format!("@echo off\r\necho %* > \"%~dp0args.txt\"\r\necho {stdout}\r\nexit /b {code}\r\n");
        fs::write(&path, body).unwrap();
        path
    }

    #[cfg(windows)]
    #[test]
    fn success_keeps_the_folder_and_passes_the_consent_flag() {
        let t = Temp::new();
        let cli = fake_cli(&t, r#"{"ok":true,"run":"20261106-090503-toy","scheduled":true}"#, 0);
        let done = start_run(&cli, &t.runs(), &request(), Duration::from_secs(20)).unwrap();
        assert_eq!(done, StartDone { run_id: "20261106-090503-toy".to_string(), scheduled: true });
        let dir = t.runs().join("20261106-090503-toy");
        assert!(dir.join("run.json").is_file() && dir.join("queue.json").is_file());
        let args = fs::read_to_string(t.0.join("args.txt")).unwrap();
        assert!(args.starts_with("start "), "{args}");
        assert!(args.contains(&*dir.to_string_lossy()) && args.contains("--json") && args.contains("--accept-unsandboxed"), "{args}");
    }

    #[cfg(windows)]
    #[test]
    fn no_consent_flag_when_the_screen_did_not_ask_for_it() {
        let t = Temp::new();
        let cli = fake_cli(&t, r#"{"ok":true,"scheduled":false}"#, 0);
        let req = StartRequest { accept_unsandboxed: false, ..request() };
        let done = start_run(&cli, &t.runs(), &req, Duration::from_secs(20)).unwrap();
        assert!(!done.scheduled);
        assert!(!fs::read_to_string(t.0.join("args.txt")).unwrap().contains("--accept-unsandboxed"));
    }

    #[cfg(windows)]
    #[test]
    fn a_refusal_shows_the_cli_text_and_leaves_no_folder() {
        let t = Temp::new();
        let cli = fake_cli(&t, r#"{"ok":false,"problems":["unit 1 bad cwd","second line"]}"#, 1);
        let why = start_run(&cli, &t.runs(), &request(), Duration::from_secs(20)).unwrap_err();
        assert_eq!(why, "unit 1 bad cwd\nsecond line");
        assert!(!t.runs().join("20261106-090503-toy").exists());
    }

    #[cfg(windows)]
    #[test]
    fn a_hung_cli_is_killed_at_the_timeout() {
        let t = Temp::new();
        let path = t.0.join("hang.cmd");
        fs::write(&path, "@echo off\r\nping -n 20 127.0.0.1 >nul\r\n").unwrap();
        let why = start_run(&path, &t.runs(), &request(), Duration::from_secs(1)).unwrap_err();
        assert!(why.contains("멈췄습니다"), "{why}");
        assert!(!t.runs().join("20261106-090503-toy").exists());
    }

    #[test]
    fn a_missing_exe_is_reported_and_leaves_no_folder() {
        let t = Temp::new();
        let why = start_run(&t.0.join("none.exe"), &t.runs(), &request(), Duration::from_secs(5)).unwrap_err();
        assert!(why.contains("띄우지 못했습니다"), "{why}");
        assert!(!t.runs().join("20261106-090503-toy").exists());
    }

    #[test]
    fn invalid_requests_write_nothing() {
        let t = Temp::new();
        let bad = StartRequest { run_id: "..".to_string(), ..request() };
        assert!(start_run(&t.0.join("none.exe"), &t.runs(), &bad, Duration::from_secs(5)).is_err());
        assert!(!t.runs().exists());
    }
}
