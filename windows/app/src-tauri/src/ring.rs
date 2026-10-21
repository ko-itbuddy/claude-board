//! 터미널 출력 고리 버퍼와 보내기 흐름 제어(크레딧) — OS 에 기대지 않는 순수 부분.
//!
//! 읽기 스레드는 웹뷰를 기다리지 않고 [`Stream::push`] 로 바이트를 쏟아 넣는다. 오래된 바이트는 고리가 덮어쓴다.
//! 보내는 쪽(배처)은 [`Stream::attach`] 마다 스레드 하나가 돌며, 웹뷰가 처리했다고 알려 준(ack) 끝 오프셋보다
//! 1MiB 넘게 앞서가지 않는다. 고리가 아직 안 보낸 바이트를 덮으면 `Gap` 을 보내고 가장 오래된 바이트부터 다시 보낸다.
//! 오프셋(`seq`)은 터미널이 태어난 뒤 쓴 총 바이트 수이고 줄지 않는다.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// 터미널 하나의 출력 고리 크기.
pub const RING_CAP: usize = 4 << 20;
/// 한 번에 보내는 최대 바이트(프레임 머리 8바이트는 따로).
pub const FRAME_MAX: usize = 64 << 10;
/// 처리 확인(ack)이 안 된 채 보낼 수 있는 바이트 한도 — 이 미만일 때만 더 보낸다.
pub const WINDOW: u64 = 1 << 20;
/// 데이터가 적을 때 모아서 보내기까지 기다리는 시간.
const FLUSH: Duration = Duration::from_millis(8);

/// 고리가 덮어써 버려 읽을 수 없는 자리를 읽으려 했다. `oldest` 부터는 읽을 수 있다.
#[derive(Debug, PartialEq, Eq)]
pub struct Gap {
    pub oldest: u64,
}

/// 오프셋이 붙은 고리 버퍼. 용량 이상을 쓰면 가장 오래된 바이트부터 사라진다.
pub struct Ring {
    buf: Vec<u8>,
    total: u64,
}

impl Ring {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "고리 용량은 0 보다 커야 한다");
        Ring { buf: vec![0; cap], total: 0 }
    }

    /// 아직 읽을 수 있는 가장 오래된 오프셋.
    pub fn oldest(&self) -> u64 {
        self.total.saturating_sub(self.buf.len() as u64)
    }

    /// 다음에 쓰일 바이트의 오프셋(= 지금까지 쓴 총 바이트).
    pub fn next(&self) -> u64 {
        self.total
    }

    pub fn push(&mut self, data: &[u8]) {
        let cap = self.buf.len();
        // 용량보다 큰 덩어리는 끝 `cap` 바이트만 남는다
        let keep = &data[data.len().saturating_sub(cap)..];
        let start = ((self.total + (data.len() - keep.len()) as u64) % cap as u64) as usize;
        let first = keep.len().min(cap - start);
        self.buf[start..start + first].copy_from_slice(&keep[..first]);
        self.buf[..keep.len() - first].copy_from_slice(&keep[first..]);
        self.total += data.len() as u64;
    }

    /// `seq` 부터 최대 `max` 바이트를 `out` 뒤에 붙이고 붙인 길이를 돌려준다. 이미 덮인 자리면 `Gap`.
    /// `seq` 가 끝에 닿았거나 넘으면 0바이트.
    pub fn read_into(&self, seq: u64, max: usize, out: &mut Vec<u8>) -> Result<usize, Gap> {
        if seq < self.oldest() {
            return Err(Gap { oldest: self.oldest() });
        }
        let n = (self.total.saturating_sub(seq) as usize).min(max);
        let cap = self.buf.len();
        let start = (seq % cap as u64) as usize;
        let first = n.min(cap - start);
        out.extend_from_slice(&self.buf[start..start + first]);
        out.extend_from_slice(&self.buf[..n - first]);
        Ok(n)
    }
}

/// 웹뷰로 보낼 한 덩어리.
#[derive(Debug, PartialEq, Eq)]
pub enum Out {
    /// 8바이트 little-endian `seq`(첫 페이로드 바이트의 오프셋) + 페이로드(최대 [`FRAME_MAX`]).
    Data(Vec<u8>),
    /// 웹뷰가 놓친 바이트가 있다. 터미널을 비우고 `seq` 부터 다시 받는다.
    Gap(u64),
    /// 셸이 끝났고 마지막 출력까지 다 보냈다.
    Exit(u32),
}

/// 덩어리를 받는 쪽. 받을 수 없으면(채널이 닫힘) false — 그 배처는 멈춘다.
pub type Sink = Arc<dyn Fn(Out) -> bool + Send + Sync>;

/// 지금 읽을 수 있는 오프셋 범위.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub oldest: u64,
    pub next: u64,
}

struct State {
    ring: Ring,
    /// 읽기 스레드가 EOF 에 닿았다(= 더 쏟아질 출력이 없다).
    output_done: bool,
    exit_code: Option<u32>,
    /// attach 마다 올라간다 — 지난 배처는 이 값이 달라지면 멈춘다.
    gen: u64,
    /// 현재 배처가 다음에 보낼 오프셋.
    sent: u64,
    /// 웹뷰가 처리했다고 알려 준 끝 오프셋.
    acked: u64,
}

/// 터미널 하나의 출력 흐름 — 읽기 스레드가 쓰고, attach 한 배처가 보낸다.
pub struct Stream {
    st: Mutex<State>,
    cv: Condvar,
}

impl Stream {
    pub fn new(cap: usize) -> Self {
        Stream {
            st: Mutex::new(State { ring: Ring::new(cap), output_done: false, exit_code: None, gen: 0, sent: 0, acked: 0 }),
            cv: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.st.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 읽기 스레드: 새 출력 바이트.
    pub fn push(&self, data: &[u8]) {
        self.lock().ring.push(data);
        self.cv.notify_all();
    }

    /// 읽기 스레드: 파이프 끝(EOF). 이 뒤에야 `Exit` 이 나갈 수 있다.
    pub fn finish_output(&self) {
        self.lock().output_done = true;
        self.cv.notify_all();
    }

    /// 웨이터: 셸 프로세스의 종료 코드.
    pub fn set_exit(&self, code: u32) {
        self.lock().exit_code = Some(code);
        self.cv.notify_all();
    }

    pub fn exit_code(&self) -> Option<u32> {
        self.lock().exit_code
    }

    /// 종료 코드가 정해질 때까지 `timeout` 만큼 기다린다. 정해졌으면 true.
    pub fn wait_exit(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut g = self.lock();
        while g.exit_code.is_none() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            g = self.cv.wait_timeout(g, left).unwrap_or_else(PoisonError::into_inner).0;
        }
        true
    }

    pub fn bounds(&self) -> Bounds {
        let g = self.lock();
        Bounds { oldest: g.ring.oldest(), next: g.ring.next() }
    }

    /// 웹뷰가 `seq` 까지 처리했다. 보낸 것보다 앞설 수 없고 뒤로 물러서지 않는다.
    pub fn ack(&self, seq: u64) {
        let mut g = self.lock();
        g.acked = g.acked.max(seq.min(g.sent));
        drop(g);
        self.cv.notify_all();
    }

    /// 보는 쪽을 `sink` 로 정하고 `from_seq` 부터 보내기 시작한다(이전 보는 쪽은 대체되고 크레딧은 처음부터).
    /// `from_seq` 가 0 이면 처음 받는 쪽이라 고리에 남은 가장 오래된 바이트부터 보낸다.
    /// 0 이 아닌데 이미 덮여 있으면 `Gap` 을 먼저 보낸다.
    pub fn attach(self: &Arc<Self>, from_seq: u64, sink: Sink) -> Bounds {
        let mut g = self.lock();
        g.gen += 1;
        let gen = g.gen;
        let b = Bounds { oldest: g.ring.oldest(), next: g.ring.next() };
        g.sent = if from_seq == 0 { b.oldest } else { from_seq.min(b.next) };
        g.acked = g.sent;
        drop(g);
        // 지난 배처를 깨워 멈추게 한다
        self.cv.notify_all();
        let me = Arc::clone(self);
        std::thread::Builder::new()
            .name("term-batch".into())
            .spawn(move || me.batcher(gen, sink))
            .expect("배처 스레드를 만들지 못했다");
        b
    }

    /// 보낼 것이 있을 때 8ms 또는 64KiB 마다 보내고, ack 안 된 바이트가 [`WINDOW`] 이상이면 기다린다.
    fn batcher(&self, gen: u64, sink: Sink) {
        let mut g = self.lock();
        let mut since: Option<Instant> = None;
        loop {
            if g.gen != gen {
                return;
            }
            let (oldest, next) = (g.ring.oldest(), g.ring.next());
            let avail = next.saturating_sub(g.sent);
            let (out, last) = if g.sent < oldest {
                g.sent = oldest;
                g.acked = oldest;
                since = None;
                (Out::Gap(oldest), false)
            } else if avail == 0 {
                since = None;
                match g.exit_code.filter(|_| g.output_done) {
                    Some(code) => (Out::Exit(code), true),
                    None => {
                        g = self.cv.wait(g).unwrap_or_else(PoisonError::into_inner);
                        continue;
                    }
                }
            } else if g.sent - g.acked >= WINDOW {
                g = self.cv.wait(g).unwrap_or_else(PoisonError::into_inner);
                continue;
            } else {
                let t0 = *since.get_or_insert_with(Instant::now);
                let full = avail >= FRAME_MAX as u64;
                let left = FLUSH.saturating_sub(t0.elapsed());
                if !full && !g.output_done && !left.is_zero() {
                    g = self.cv.wait_timeout(g, left).unwrap_or_else(PoisonError::into_inner).0;
                    continue;
                }
                let mut frame = Vec::with_capacity(8 + FRAME_MAX.min(avail as usize));
                frame.extend_from_slice(&g.sent.to_le_bytes());
                let n = g.ring.read_into(g.sent, FRAME_MAX, &mut frame).expect("오프셋 검사를 이미 했다");
                g.sent += n as u64;
                since = None;
                (Out::Data(frame), false)
            };
            // 보내는 동안 락을 쥐지 않는다 — 읽기 스레드와 ack 가 막히면 안 된다
            drop(g);
            if !sink(out) || last {
                return;
            }
            g = self.lock();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(r: &Ring, seq: u64, max: usize) -> Result<Vec<u8>, Gap> {
        let mut v = Vec::new();
        r.read_into(seq, max, &mut v).map(|_| v)
    }

    #[test]
    fn ring_keeps_order_across_wrap() {
        let mut r = Ring::new(8);
        r.push(b"abc");
        r.push(b"def");
        assert_eq!((r.oldest(), r.next()), (0, 6));
        assert_eq!(read(&r, 0, 100).unwrap(), b"abcdef");
        // 용량 8 을 넘겨 한 바퀴 돈다
        r.push(b"ghijk");
        assert_eq!((r.oldest(), r.next()), (3, 11));
        assert_eq!(read(&r, 3, 100).unwrap(), b"defghijk");
        assert_eq!(read(&r, 7, 100).unwrap(), b"hijk");
        assert_eq!(read(&r, 5, 2).unwrap(), b"fg");
    }

    #[test]
    fn ring_drops_oldest_and_reports_gap() {
        let mut r = Ring::new(4);
        r.push(b"0123456789");
        assert_eq!((r.oldest(), r.next()), (6, 10));
        assert_eq!(read(&r, 6, 100).unwrap(), b"6789");
        assert_eq!(read(&r, 0, 100), Err(Gap { oldest: 6 }));
        assert_eq!(read(&r, 5, 100), Err(Gap { oldest: 6 }));
        // 끝이거나 넘으면 빈 값
        assert_eq!(read(&r, 10, 100).unwrap(), b"");
        assert_eq!(read(&r, 99, 100).unwrap(), b"");
    }

    #[test]
    fn ring_oversized_chunk_then_small_ones_stay_contiguous() {
        let mut r = Ring::new(5);
        r.push(b"abcdefghijkl"); // 12바이트, 남는 것은 hijkl
        r.push(b"mn");
        assert_eq!(read(&r, r.oldest(), 100).unwrap(), b"jklmn");
        assert_eq!((r.oldest(), r.next()), (9, 14));
    }

    /// 시험용 받는 쪽 — 덩어리를 모으고, 요청하면 ack 한다.
    #[derive(Default)]
    struct Cap {
        outs: Mutex<Vec<Out>>,
    }

    impl Cap {
        fn sink(self: &Arc<Self>) -> Sink {
            let me = Arc::clone(self);
            Arc::new(move |o| {
                me.outs.lock().unwrap().push(o);
                true
            })
        }

        fn data_bytes(&self) -> u64 {
            bytes_of(&self.outs.lock().unwrap())
        }

        fn wait(&self, what: &str, f: impl Fn(&[Out]) -> bool) {
            let end = Instant::now() + Duration::from_secs(10);
            while !f(&self.outs.lock().unwrap()) {
                assert!(Instant::now() < end, "10초 안에 \"{what}\" 가 되지 않았다");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }

    fn bytes_of(outs: &[Out]) -> u64 {
        outs.iter().map(|o| if let Out::Data(f) = o { f.len() as u64 - 8 } else { 0 }).sum()
    }

    fn seq_of(f: &[u8]) -> u64 {
        u64::from_le_bytes(f[..8].try_into().unwrap())
    }

    #[test]
    fn stream_sends_contiguous_frames_then_exit_last() {
        let s = Arc::new(Stream::new(1 << 16));
        let cap = Arc::new(Cap::default());
        s.push(b"hello ");
        let b = s.attach(0, cap.sink());
        assert_eq!((b.oldest, b.next), (0, 6));
        s.push(b"world");
        cap.wait("11바이트", |o| bytes_of(o) == 11);
        // 종료 코드만 있고 출력이 안 끝났으면 Exit 은 아직이다
        s.set_exit(3);
        std::thread::sleep(Duration::from_millis(60));
        assert!(!cap.outs.lock().unwrap().iter().any(|o| matches!(o, Out::Exit(_))));
        s.push(b"!");
        s.finish_output();
        cap.wait("Exit", |o| matches!(o.last(), Some(Out::Exit(3))));
        let outs = cap.outs.lock().unwrap();
        let (mut at, mut text) = (0u64, Vec::new());
        for o in outs.iter() {
            if let Out::Data(f) = o {
                assert_eq!(seq_of(f), at, "프레임 오프셋이 이어지지 않는다");
                at += f.len() as u64 - 8;
                text.extend_from_slice(&f[8..]);
            }
        }
        assert_eq!(text, b"hello world!");
        assert_eq!(outs.iter().filter(|o| matches!(o, Out::Exit(_))).count(), 1);
    }

    #[test]
    fn attach_from_zero_after_overwrite_starts_at_oldest_without_gap() {
        let s = Arc::new(Stream::new(100));
        s.push(&[7u8; 250]);
        let cap = Arc::new(Cap::default());
        s.attach(0, cap.sink());
        cap.wait("100바이트", |o| bytes_of(o) == 100);
        let outs = cap.outs.lock().unwrap();
        assert_eq!(outs.len(), 1);
        let Out::Data(f) = &outs[0] else { panic!("데이터가 아니다: {:?}", outs[0]) };
        assert_eq!(seq_of(f), 150);
    }

    #[test]
    fn attach_from_nonzero_inside_overwritten_area_gets_gap_first() {
        let s = Arc::new(Stream::new(100));
        s.push(&[1u8; 250]);
        let cap = Arc::new(Cap::default());
        s.attach(10, cap.sink());
        cap.wait("Gap 과 100바이트", |o| bytes_of(o) == 100);
        let outs = cap.outs.lock().unwrap();
        assert_eq!(outs[0], Out::Gap(150));
        let Out::Data(f) = &outs[1] else { panic!("Gap 다음이 데이터가 아니다") };
        assert_eq!(seq_of(f), 150);
    }

    #[test]
    fn slow_viewer_overwritten_midstream_gets_gap_and_resumes_at_oldest() {
        let s = Arc::new(Stream::new(128 << 10));
        let gate = Arc::new(Mutex::new(()));
        let cap = Arc::new(Cap::default());
        let (c, g) = (Arc::clone(&cap), Arc::clone(&gate));
        let sink: Sink = Arc::new(move |o| {
            let _held = g.lock().unwrap();
            c.outs.lock().unwrap().push(o);
            true
        });
        let hold = gate.lock().unwrap();
        s.push(&[1u8; 70_000]);
        s.attach(0, sink);
        std::thread::sleep(Duration::from_millis(50));
        // 첫 프레임이 받는 쪽에서 막힌 사이 고리가 한 바퀴 넘게 돈다
        for _ in 0..10 {
            s.push(&[2u8; 64 << 10]);
        }
        drop(hold);
        let end = s.bounds().next;
        cap.wait("Gap 뒤 끝까지", |o| {
            o.iter().any(|x| matches!(x, Out::Gap(_)))
                && o.iter().rev().find_map(|x| if let Out::Data(f) = x { Some(seq_of(f) + f.len() as u64 - 8) } else { None }) == Some(end)
        });
        let outs = cap.outs.lock().unwrap();
        let gap_at = outs.iter().position(|x| matches!(x, Out::Gap(_))).unwrap();
        let Out::Gap(oldest) = outs[gap_at] else { unreachable!() };
        let Out::Data(next) = &outs[gap_at + 1] else { panic!("Gap 다음이 데이터가 아니다") };
        assert_eq!(seq_of(next), oldest, "Gap 뒤에는 가장 오래된 바이트부터 다시 온다");
    }

    #[test]
    fn unacked_bytes_stop_at_window_and_ack_releases_more() {
        let s = Arc::new(Stream::new(RING_CAP));
        let cap = Arc::new(Cap::default());
        s.push(&vec![9u8; 3 << 20]);
        s.attach(0, cap.sink());
        cap.wait("창이 찰 때까지", |o| bytes_of(o) >= WINDOW);
        std::thread::sleep(Duration::from_millis(80));
        let held = cap.data_bytes();
        assert!(held >= WINDOW && held < WINDOW + FRAME_MAX as u64, "ack 없이 보낸 바이트 {held}");
        s.ack(held);
        cap.wait("더 보냄", |o| bytes_of(o) > held);
        // 계속 ack 하면 끝까지 간다
        let end = Instant::now() + Duration::from_secs(10);
        while cap.data_bytes() < 3 << 20 {
            assert!(Instant::now() < end, "3MiB 가 다 가지 않았다: {}", cap.data_bytes());
            s.ack(cap.data_bytes());
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn new_attach_replaces_old_viewer_and_resets_credit() {
        let s = Arc::new(Stream::new(1 << 16));
        let (a, b) = (Arc::new(Cap::default()), Arc::new(Cap::default()));
        s.push(b"one");
        s.attach(0, a.sink());
        a.wait("첫 보는 쪽이 3바이트", |o| bytes_of(o) == 3);
        s.attach(0, b.sink());
        b.wait("둘째 보는 쪽이 처음부터 3바이트", |o| bytes_of(o) == 3);
        s.push(b"two");
        b.wait("둘째가 6바이트", |o| bytes_of(o) == 6);
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(a.data_bytes(), 3, "대체된 보는 쪽에는 더 보내지 않는다");
    }

    #[test]
    fn exit_is_replayed_to_a_viewer_that_attaches_afterwards() {
        let s = Arc::new(Stream::new(1 << 16));
        s.push(b"bye");
        s.set_exit(0);
        s.finish_output();
        let cap = Arc::new(Cap::default());
        s.attach(0, cap.sink());
        cap.wait("Exit", |o| matches!(o.last(), Some(Out::Exit(0))));
        assert_eq!(cap.data_bytes(), 3);
    }
}
