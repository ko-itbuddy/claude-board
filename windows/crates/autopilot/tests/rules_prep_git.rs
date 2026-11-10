//! (청크 PrepGit 1/2) 출발 준비 git 자리 확인(`rules::prep_git`) 시험 — 전반.
//! 원본: `Tests/AutopilotRulesTests/PrepGitTests.swift`. 이 파일이 옮긴 Swift 시험: `읽기env` · `물려받은env`(같은 이름·같은 글).
//! 남은 Swift 시험(R-PrepGit-b): 가짜 파일 시스템 `FakeGitFS` · 표 `rows`(시험 `확인`) · `공통폴더박기` · `글`(`apPrepGitBlockText` 와 `apPrepShortPath` 를 함께 보므로 통째로 b).
//! 끝의 「추가」 절은 Swift 시험에 없는 이 이식의 몫(Windows 경로·환경 이름·원본 동작 고정)이다.

use claude_board_autopilot::rules::prep_git::{
    fsmonitor_is_bool, lexical, lfs_filter, prep_git_env, short_path, ConfigEntry, GitEntry, PrepGit, PREP_GIT_READ_ENV,
};
use std::collections::BTreeMap;

fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn strs(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn 읽기env() {
    assert_eq!(PREP_GIT_READ_ENV.to_vec(), vec!["GIT_NO_LAZY_FETCH=1", "GIT_CONFIG_COUNT=1", "GIT_CONFIG_KEY_0=protocol.allow", "GIT_CONFIG_VALUE_0=never"]);
}

#[test]
fn 물려받은env() {
    let inherited = map(&[
        ("PATH", "/usr/bin:/bin"),
        ("HOME", "/h"),
        ("GIT_CONFIG_PARAMETERS", "'filter.q.clean'='/w/plant.sh'"),
        ("GIT_CONFIG_COUNT", "2"),
        ("GIT_CONFIG_KEY_0", "core.fsmonitor"),
        ("GIT_CONFIG_VALUE_0", "/w/plant.sh"),
        ("GIT_CONFIG_KEY_1", "x"),
        ("GIT_CONFIG_VALUE_1", "y"),
        ("GIT_SSH_COMMAND", "/w/plant.sh"),
        ("GIT_ALLOW_PROTOCOL", "file"),
        ("GIT_INDEX_FILE", "/tmp/i"),
        ("GIT_DIR", "/elsewhere"),
    ]);
    let pin = PrepGit { top: Some("/w/a".into()), git_dir: Some("/w/a/.git".into()), common_dir: Some("/w/a/.git".into()), block: None }.pin();
    let env = prep_git_env(&inherited, &pin);
    assert_eq!(
        env,
        map(&[
            ("PATH", "/usr/bin:/bin"),
            ("HOME", "/h"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_ATTR_NOSYSTEM", "1"),
            ("GIT_NO_REPLACE_OBJECTS", "1"),
            ("GIT_DIR", "/w/a/.git"),
            ("GIT_COMMON_DIR", "/w/a/.git"),
            ("GIT_WORK_TREE", "/w/a"),
            ("GIT_NO_LAZY_FETCH", "1"),
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "protocol.allow"),
            ("GIT_CONFIG_VALUE_0", "never"),
        ])
    );
}

// ---- 추가: 원본 동작 고정(Swift 시험에 없음) ----

#[test]
fn 추가_pin_은_통과한_저장소만_박는다() {
    let ok = PrepGit { top: Some("/w/wt/x".into()), git_dir: Some("/w/a/.git/worktrees/x".into()), common_dir: Some("/w/a/.git".into()), block: None };
    let mut want = strs(&["GIT_DIR=/w/a/.git/worktrees/x", "GIT_COMMON_DIR=/w/a/.git", "GIT_WORK_TREE=/w/wt/x"]);
    want.extend(PREP_GIT_READ_ENV.iter().map(|s| s.to_string()));
    assert_eq!(ok.pin(), want);
    // commonDir 가 없으면 git 폴더를 공통으로 박는다
    let plain = PrepGit { top: Some("/w/a".into()), git_dir: Some("/w/a/.git".into()), ..Default::default() };
    assert_eq!(plain.pin()[1], "GIT_COMMON_DIR=/w/a/.git");
    // 막혔거나 뿌리·git 폴더를 모르면 비어 있다
    assert!(PrepGit { block: Some("까닭".into()), ..ok.clone() }.pin().is_empty());
    assert!(PrepGit { git_dir: None, ..ok.clone() }.pin().is_empty());
    assert!(PrepGit { top: None, ..ok }.pin().is_empty());
    assert!(PrepGit::default().pin().is_empty());
}

#[test]
fn 추가_모양_값() {
    assert_eq!(GitEntry::File(None), GitEntry::File(None));
    assert_ne!(GitEntry::Dir, GitEntry::Link);
    assert_eq!(ConfigEntry::new("core.fsmonitor", None), ConfigEntry { key: "core.fsmonitor".into(), value: None });
    assert_eq!(ConfigEntry::new("a.b", Some("c")).value.as_deref(), Some("c"));
}

#[test]
fn 추가_env_extra_는_첫_등호에서_가르고_등호_없으면_버린다() {
    let env = prep_git_env(&map(&[("PATH", "p")]), &strs(&["GIT_DIR=/a=b", "NOEQ", "=x"]));
    assert_eq!(env.get("GIT_DIR").map(String::as_str), Some("/a=b"));
    assert!(!env.contains_key("NOEQ"));
    assert_eq!(env.get("").map(String::as_str), Some("x"));
    // 박는 값은 보호 값을 덮을 수 있다(원본 순서: 보호 뒤에 extra)
    let over = prep_git_env(&map(&[]), &strs(&["GIT_CONFIG_GLOBAL=/x"]));
    assert_eq!(over.get("GIT_CONFIG_GLOBAL").map(String::as_str), Some("/x"));
}

#[test]
fn 추가_env_git_접두만_뺀다() {
    // GITHUB_TOKEN 은 `GIT_` 접두가 아니다 — 원본도 남긴다
    let env = prep_git_env(&map(&[("GITHUB_TOKEN", "t"), ("GIT", "x"), ("MY_GIT_X", "y"), ("GIT_X", "z")]), &[]);
    assert_eq!(env.get("GITHUB_TOKEN").map(String::as_str), Some("t"));
    assert_eq!(env.get("GIT").map(String::as_str), Some("x"));
    assert_eq!(env.get("MY_GIT_X").map(String::as_str), Some("y"));
    assert!(!env.contains_key("GIT_X"));
}

#[test]
fn 추가_fsmonitor_불리언_꼴() {
    for v in [None, Some(""), Some("true"), Some("FALSE"), Some("Yes"), Some("no"), Some("ON"), Some("off"), Some("1"), Some("0"), Some("  true\t"), Some("\u{3000}off"), Some("\u{a0}1")] {
        assert!(fsmonitor_is_bool(v), "{v:?}");
    }
    for v in ["/w/cfg/plant.sh", "touch x", "watchman-hook", "~/hook", "2", "truee", "tr ue", "false\n", "false\r", "\ntrue", "\u{200b}true"] {
        assert!(!fsmonitor_is_bool(Some(v)), "{v:?}");
    }
}

#[test]
fn 추가_lfs_표준글_표() {
    assert_eq!(lfs_filter("filter.lfs.clean"), Some("git-lfs clean -- %f"));
    assert_eq!(lfs_filter("filter.lfs.smudge"), Some("git-lfs smudge -- %f"));
    assert_eq!(lfs_filter("filter.lfs.process"), Some("git-lfs filter-process"));
    assert_eq!(lfs_filter("filter.lfs.required"), None);
    assert_eq!(lfs_filter("filter.x.clean"), None);
    // 키는 소문자로만 찾는다(git 이 소문자로 주는 키를 받는 쪽이 접는다)
    assert_eq!(lfs_filter("filter.LFS.clean"), None);
}

#[test]
fn 추가_lexical_posix() {
    assert_eq!(lexical("/"), "/");
    assert_eq!(lexical("/w/a"), "/w/a");
    assert_eq!(lexical("/w/a/"), "/w/a");
    assert_eq!(lexical("/w//a/./b"), "/w/a/b");
    assert_eq!(lexical("/w/a/../b"), "/w/b");
    assert_eq!(lexical("/.."), "/");
    assert_eq!(lexical("/w/../../x"), "/x");
    assert_eq!(lexical("//tmp/x"), "/tmp/x");
    assert_eq!(lexical("/h/.claude-board/worktrees/../worktrees"), "/h/.claude-board/worktrees");
    // 대소문자는 접지 않는다(견주는 쪽이 접는다)
    assert_eq!(lexical("/W/A/team.gitconfig"), "/W/A/team.gitconfig");
    // 상대 경로: 작업 폴더로 풀지 않고 정리만 한다
    assert_eq!(lexical("a/./b/../c"), "a/c");
    // POSIX 에서 `\` 는 파일 이름 글자다
    assert_eq!(lexical(r"/w/a\b"), r"/w/a\b");
}

#[test]
fn 추가_lexical_windows() {
    assert_eq!(lexical(r"C:\w\a"), "C:/w/a");
    assert_eq!(lexical("C:/w/a"), "C:/w/a");
    assert_eq!(lexical(r"C:\w\a\"), "C:/w/a");
    assert_eq!(lexical(r"C:\w\a\..\b"), "C:/w/b");
    assert_eq!(lexical(r"C:\w/a\.\b"), "C:/w/a/b");
    assert_eq!(lexical(r"C:\w\\a"), "C:/w/a");
    assert_eq!(lexical(r"C:\"), "C:/");
    assert_eq!(lexical(r"C:\.."), "C:/");
    assert_eq!(lexical(r"C:\w\..\..\x"), "C:/x");
    assert_eq!(lexical("C:"), "C:");
    // 확장 길이·장치 접두
    assert_eq!(lexical(r"\\?\C:\w\a"), "C:/w/a");
    assert_eq!(lexical(r"\\.\C:\w\a"), "C:/w/a");
    assert_eq!(lexical(r"\\?\C:\w\..\x"), "C:/x");
    // UNC
    assert_eq!(lexical(r"\\srv\sh\w\a"), "//srv/sh/w/a");
    assert_eq!(lexical(r"\\srv\sh"), "//srv/sh/");
    assert_eq!(lexical(r"\\srv\sh\..\..\x"), "//srv/sh/x");
    assert_eq!(lexical(r"\\?\UNC\srv\sh\w"), "//srv/sh/w");
    assert_eq!(lexical(r"\\?\unc\srv\sh\w"), "//srv/sh/w");
    // 뿌리 없는 `\x`(현재 드라이브)
    assert_eq!(lexical(r"\w\a"), "/w/a");
    // 윈도가 무시하는 끝 점·공백: 같은 자리가 같은 글이 된다
    assert_eq!(lexical(r"C:\w\a.\b "), "C:/w/a/b");
    assert_eq!(lexical(r"C:\w\a. .\x"), "C:/w/a/x");
    assert_eq!(lexical(r"C:\w\...\x"), "C:/w/x");
    // 견주기: 접두 형이 달라도 소문자로 접으면 같은 자리
    assert_eq!(lexical(r"\\?\c:\W\A").to_lowercase(), lexical(r"C:\w\a").to_lowercase());
    assert_eq!(lexical(r"\\?\UNC\SRV\Sh\W").to_lowercase(), lexical(r"\\srv\sh\w").to_lowercase());
    // 한글·긴 이름이 든 경로도 잘리지 않는다
    assert_eq!(lexical(r"C:\사용자\홍길동\작업"), "C:/사용자/홍길동/작업");
    assert_eq!(lexical(r"\\?\UNC\서버\공유\작업"), "//서버/공유/작업");
}

#[test]
fn 추가_lexical_는_짧거나_깨진_접두에서_죽지_않는다() {
    for p in ["", "\\", "\\\\", r"\\?\", r"\\?\UNC", r"\\?\UNC\", r"\\?\U", r"\\?\한", r"\\?\UN한", r"\\.\", "C:\\\\", "x:", "ab:\\"] {
        let _ = lexical(p);
    }
    assert_eq!(lexical(r"\\?\한글\x"), "/한글/x");
}

#[test]
fn 추가_short_path_posix() {
    assert_eq!(short_path("/h/w/nest/app", "/h"), "~/w/nest/app");
    assert_eq!(short_path("/opt/x", "/h"), "/opt/x");
    assert_eq!(short_path("/h", "/h"), "/h");
    assert_eq!(short_path("/hx/w", "/h"), "/hx/w");
    // 홈이 POSIX 꼴이면 `\` 는 구분자가 아니다
    assert_eq!(short_path(r"/h\x", "/h"), r"/h\x");
    // 대소문자를 가리지 않는 것은 윈도 꼴 홈뿐이다
    assert_eq!(short_path("/H/w", "/h"), "/H/w");
}

#[test]
fn 추가_short_path_windows() {
    let home = r"C:\Users\x";
    assert_eq!(short_path(r"C:\Users\x\w\a", home), r"~\w\a");
    assert_eq!(short_path(r"c:\users\X\w", home), r"~\w");
    assert_eq!(short_path("C:/Users/x/w", home), "~/w");
    assert_eq!(short_path(r"C:\Users\xy\w", home), r"C:\Users\xy\w");
    assert_eq!(short_path(r"C:\Users\x", home), r"C:\Users\x");
    assert_eq!(short_path(r"D:\w", home), r"D:\w");
    assert_eq!(short_path(r"\\srv\sh\w", r"\\srv\sh"), r"~\w");
    // 한글 이름 한가운데에서 자르지 않는다
    assert_eq!(short_path(r"C:\사용자\w", home), r"C:\사용자\w");
}

#[test]
fn 추가_env_윈도는_환경_이름_대소문자를_가리지_않는다() {
    let inherited = map(&[
        ("Path", r"C:\Windows"),
        ("git_config_parameters", "'filter.q.clean'='x'"),
        ("Git_Dir", r"C:\elsewhere"),
        ("GIT_SSH_COMMAND", "x"),
        ("git_ssh", "x"),
        ("USERPROFILE", r"C:\Users\x"),
    ]);
    let env = prep_git_env(&inherited, &[]);
    assert_eq!(
        env,
        map(&[
            ("Path", r"C:\Windows"),
            ("USERPROFILE", r"C:\Users\x"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_ATTR_NOSYSTEM", "1"),
            ("GIT_NO_REPLACE_OBJECTS", "1"),
        ])
    );
}

#[test]
fn 추가_pin_은_윈도_경로를_글자_그대로_박는다() {
    let g = PrepGit { top: Some(r"C:\w\a".into()), git_dir: Some(r"C:\w\a\.git".into()), common_dir: None, block: None };
    let pin = g.pin();
    assert_eq!(&pin[..3], strs(&[r"GIT_DIR=C:\w\a\.git", r"GIT_COMMON_DIR=C:\w\a\.git", r"GIT_WORK_TREE=C:\w\a"]).as_slice());
}
