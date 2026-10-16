//! 보드 창 — 실행 크기·오른쪽에 붙이기·최대축소·항상 위·보이기 (SPEC §5.10, §7, `ClaudeBoard.swift:4246-4475, 4934-5014`).
//! 위치·크기는 모두 물리 픽셀로 다룬다(모니터마다 배율이 다를 수 있어 그 모니터의 배율을 곱한다).

use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::settings::Frame;

/// 실행할 때의 가로 폭(논리 픽셀)
pub const LAUNCH_WIDTH: f64 = 454.0;
/// 전체·중간 화면이 무너지지 않는 폭 — 더 좁으면 미니로 그린다
pub const BOARD_MIN_WIDTH: f64 = 360.0;
pub const MINI_WIDTH: f64 = 300.0;
pub const MINI_MIN_HEIGHT: f64 = 144.0;
pub const MINI_ROW_HEIGHT: f64 = 26.0;

/// 모니터의 작업 영역(작업 표시줄 제외, 물리 픽셀)과 배율.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mon {
    pub work: Frame,
    pub scale: f64,
}

pub fn intersects(a: Frame, b: Frame) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

fn contains(m: &Mon, x: f64, y: f64) -> bool {
    x >= m.work.x && x < m.work.x + m.work.w && y >= m.work.y && y < m.work.y + m.work.h
}

/// 처음 크기로 오른쪽에 붙인 창 — 폭 454, 작업 영역 높이 전체.
pub fn snap_frame(m: &Mon) -> Frame {
    let w = LAUNCH_WIDTH * m.scale;
    Frame { x: m.work.x + m.work.w - w, y: m.work.y, w, h: m.work.h }
}

/// 실행할 때의 창. 미니로 두고 껐으면 그 크기·위치 그대로(화면 안에 있을 때), 아니면 마지막 가로 위치에
/// 폭 454·그 모니터 작업 영역 높이 전체. 저장한 위치가 없으면 주 모니터 오른쪽에 붙인다.
pub fn launch_frame(mons: &[Mon], saved: Option<Frame>, mini: bool) -> Option<Frame> {
    let first = mons.first()?;
    if let Some(s) = saved.filter(|s| mini && mons.iter().any(|m| intersects(m.work, *s))) {
        return Some(s);
    }
    let Some(s) = saved else { return Some(snap_frame(first)) };
    let m = mons
        .iter()
        .find(|m| contains(m, s.x + s.w / 2.0, s.y + s.h / 2.0))
        .or_else(|| mons.iter().find(|m| intersects(m.work, s)))
        .unwrap_or(first);
    let w = LAUNCH_WIDTH * m.scale;
    let x = s.x.clamp(m.work.x, (m.work.x + m.work.w - w).max(m.work.x));
    Some(Frame { x, y: m.work.y, w, h: m.work.h })
}

/// 최대축소 — 오른쪽 위 모서리는 그대로 두고 폭 300, 높이는 경로 수(최대 6줄)에 맞춘다(최소 높이에 두 줄).
pub fn mini_frame(cur: Frame, rows: usize, scale: f64) -> Frame {
    let h = (MINI_MIN_HEIGHT + (rows.clamp(2, 6) - 2) as f64 * MINI_ROW_HEIGHT) * scale;
    let w = MINI_WIDTH * scale;
    Frame { x: cur.x + cur.w - w, y: cur.y, w, h }
}

/// ⤢ 로 돌아갈 위치로 쓸 수 있는지 — 폭이 미니보다 넓고 지금 연결된 화면과 겹친다.
pub fn restorable(f: Frame, mons: &[Mon]) -> bool {
    mons.iter().any(|m| intersects(m.work, f) && f.w >= BOARD_MIN_WIDTH * m.scale)
}

/// 연결된 모니터들(주 모니터가 먼저).
pub fn monitors(app: &AppHandle) -> Vec<Mon> {
    let primary = app.primary_monitor().ok().flatten().map(|m| *m.position());
    let mut list: Vec<(bool, Mon)> = app
        .available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|m| {
            let a = m.work_area();
            let work = Frame { x: a.position.x as f64, y: a.position.y as f64, w: a.size.width as f64, h: a.size.height as f64 };
            (Some(*m.position()) == primary, Mon { work, scale: m.scale_factor() })
        })
        .collect();
    list.sort_by_key(|(p, _)| !*p);
    list.into_iter().map(|(_, m)| m).collect()
}

/// 창이 있는 모니터(없으면 주 모니터).
pub fn monitor_of(win: &WebviewWindow) -> Option<Mon> {
    let mons = monitors(win.app_handle());
    let f = frame(win);
    f.and_then(|f| mons.iter().find(|m| contains(m, f.x + f.w / 2.0, f.y + f.h / 2.0)).copied()).or_else(|| mons.first().copied())
}

pub fn frame(win: &WebviewWindow) -> Option<Frame> {
    let p = win.outer_position().ok()?;
    let s = win.outer_size().ok()?;
    Some(Frame { x: p.x as f64, y: p.y as f64, w: s.width as f64, h: s.height as f64 })
}

pub fn set_frame(win: &WebviewWindow, f: Frame) {
    let _ = win.set_size(PhysicalSize::new(f.w.round().max(1.0) as u32, f.h.round().max(1.0) as u32));
    let _ = win.set_position(PhysicalPosition::new(f.x.round() as i32, f.y.round() as i32));
}

pub fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window("main")
}

/// 포커스를 가져가지 않고 보인다 — `--background`·전역 단축키·Windows Terminal 실행 때.
pub fn show_inactive(win: &WebviewWindow) {
    #[cfg(windows)]
    if let Ok(h) = win.hwnd() {
        crate::win::show_no_activate(h.0 as isize);
        return;
    }
    let _ = win.show();
}

/// 보드를 보인다(최소화돼 있으면 되살린다).
pub fn show(app: &AppHandle, focus: bool) {
    let Some(win) = main_window(app) else { return };
    if win.is_minimized().unwrap_or(false) {
        let _ = win.unminimize();
    }
    if focus {
        let _ = win.show();
        let _ = win.set_focus();
    } else {
        show_inactive(&win);
    }
}

/// 트레이 왼쪽 클릭 — 보이면 숨기고, 숨었거나 최소화돼 있으면 보인다.
pub fn toggle(app: &AppHandle) {
    let Some(win) = main_window(app) else { return };
    if win.is_visible().unwrap_or(false) && !win.is_minimized().unwrap_or(false) {
        let _ = win.hide();
    } else {
        show(app, true);
    }
}

/// Windows 11 Mica, 안 되면 아크릴. 실패는 무시한다(Windows 10 등).
pub fn apply_vibrancy(win: &WebviewWindow) {
    #[cfg(windows)]
    if window_vibrancy::apply_mica(win, Some(true)).is_err() {
        let _ = window_vibrancy::apply_acrylic(win, Some((18, 18, 28, 160)));
    }
    let _ = win;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(x: f64, y: f64, w: f64, h: f64, scale: f64) -> Mon {
        Mon { work: Frame { x, y, w, h }, scale }
    }

    #[test]
    fn launch_and_snap() {
        let main = mon(0.0, 0.0, 1920.0, 1040.0, 1.0);
        let hi = mon(1920.0, 0.0, 2560.0, 1392.0, 1.5);
        // 저장한 위치가 없으면 주 모니터 오른쪽
        assert_eq!(launch_frame(&[main, hi], None, false), Some(Frame { x: 1466.0, y: 0.0, w: 454.0, h: 1040.0 }));
        // 마지막 가로 위치는 두고 폭·높이를 실행 크기로 — 그 모니터의 배율로
        let saved = Frame { x: 2000.0, y: 300.0, w: 800.0, h: 400.0 };
        assert_eq!(launch_frame(&[main, hi], Some(saved), false), Some(Frame { x: 2000.0, y: 0.0, w: 681.0, h: 1392.0 }));
        // 화면 밖으로 나가지 않게
        let edge = Frame { x: 1800.0, y: 10.0, w: 454.0, h: 900.0 };
        assert_eq!(launch_frame(&[main], Some(edge), false).unwrap().x, 1466.0);
        // 미니로 껐으면 그대로, 화면 밖이면 실행 크기로
        let m = Frame { x: 1600.0, y: 20.0, w: 300.0, h: 144.0 };
        assert_eq!(launch_frame(&[main], Some(m), true), Some(m));
        let lost = Frame { x: 9000.0, y: 20.0, w: 300.0, h: 144.0 };
        assert_eq!(launch_frame(&[main], Some(lost), true).unwrap().w, 454.0);
        assert_eq!(launch_frame(&[], None, false), None);
        assert_eq!(snap_frame(&hi), Frame { x: 1920.0 + 2560.0 - 681.0, y: 0.0, w: 681.0, h: 1392.0 });
    }

    #[test]
    fn mini_and_restore() {
        let cur = Frame { x: 1466.0, y: 0.0, w: 454.0, h: 1040.0 };
        assert_eq!(mini_frame(cur, 1, 1.0), Frame { x: 1620.0, y: 0.0, w: 300.0, h: 144.0 });
        assert_eq!(mini_frame(cur, 4, 1.0).h, 144.0 + 52.0);
        assert_eq!(mini_frame(cur, 20, 2.0), Frame { x: 1320.0, y: 0.0, w: 600.0, h: (144.0 + 104.0) * 2.0 });
        let mons = [mon(0.0, 0.0, 1920.0, 1040.0, 1.0)];
        assert!(restorable(cur, &mons));
        assert!(!restorable(Frame { w: 300.0, ..cur }, &mons));
        assert!(!restorable(Frame { x: 5000.0, ..cur }, &mons));
    }
}
