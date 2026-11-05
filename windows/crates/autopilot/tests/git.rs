//! git 어댑터 시험 — 진짜 git.exe 와 임시 저장소로 한다. git 이 없으면 건너뛰지 않고 실패한다.

use claude_board_autopilot::adapters::git::{
    autopilot_branch_name, parse_diff_stat, parse_porcelain_status, ProcessGit,
};
use claude_board_autopilot::ports::{Git, PortErrorKind};
use std::path::Path;
use std::time::Duration;

fn git() -> ProcessGit {
    ProcessGit::new()
}

/// 저장소를 만든다. 전역 설정이 꺼져 있으므로 신원은 저장소 설정에 둔다.
fn init_repo(dir: &Path) {
    let g = git();
    let out = g.run(dir, &["init", "-q", "-b", "main"]).expect("git.exe 가 있어야 한다");
    assert_eq!(out.code, 0, "init 실패: {}", out.stderr);
    for (k, v) in [("user.name", "t"), ("user.email", "t@example.com"), ("commit.gpgsign", "false")] {
        assert_eq!(g.run(dir, &["config", k, v]).unwrap().code, 0);
    }
}

fn slashes(p: &Path) -> String {
    p.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/")
}

fn commit_empty(dir: &Path) {
    let out = git().run(dir, &["commit", "--allow-empty", "-q", "-m", "base"]).unwrap();
    assert_eq!(out.code, 0, "commit 실패: {}", out.stderr);
}

#[test]
fn malicious_config_and_hook_do_not_run() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init_repo(repo);
    let marker_fs = repo.join("marker-fsmonitor");
    let marker_hook = repo.join("marker-hook");

    // 훅과 fsmonitor 가 표식 파일을 만든다(경로는 / 로 써서 sh 에서 깨지지 않게)
    let fs_path = slashes(&marker_fs);
    let hook_path = slashes(&marker_hook);
    let cfg = repo.join(".git").join("config");
    let mut text = std::fs::read_to_string(&cfg).unwrap();
    text.push_str(&format!("[core]\n\tfsmonitor = \"echo x > '{fs_path}'; exit 0\"\n"));
    std::fs::write(&cfg, text).unwrap();
    let hooks = repo.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    std::fs::write(hooks.join("pre-commit"), format!("#!/bin/sh\necho x > '{hook_path}'\nexit 0\n")).unwrap();

    let g = git();
    assert_eq!(g.run(repo, &["status"]).unwrap().code, 0);
    let c = g.run(repo, &["commit", "--allow-empty", "-q", "-m", "x"]).unwrap();
    assert_eq!(c.code, 0, "commit 실패: {}", c.stderr);

    assert!(!marker_fs.exists(), "fsmonitor 명령이 돌았다");
    assert!(!marker_hook.exists(), "pre-commit 훅이 돌았다");
}

#[test]
fn global_and_inherited_git_env_are_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init_repo(repo);
    // 부모 환경의 GIT_DIR 이 엉뚱한 곳을 가리켜도 -C 의 저장소를 본다
    let other = tempfile::tempdir().unwrap();
    std::env::set_var("GIT_DIR", other.path());
    std::env::set_var("GIT_CONFIG_COUNT", "1");
    std::env::set_var("GIT_CONFIG_KEY_0", "core.bare");
    std::env::set_var("GIT_CONFIG_VALUE_0", "true");
    let out = git().run(repo, &["rev-parse", "--is-bare-repository", "--absolute-git-dir"]).unwrap();
    std::env::remove_var("GIT_DIR");
    std::env::remove_var("GIT_CONFIG_COUNT");
    std::env::remove_var("GIT_CONFIG_KEY_0");
    std::env::remove_var("GIT_CONFIG_VALUE_0");
    assert_eq!(out.code, 0, "{}", out.stderr);
    let mut lines = out.stdout.lines();
    assert_eq!(lines.next(), Some("false"));
    assert!(lines.next().unwrap().ends_with("/.git"), "{}", out.stdout);
}

#[test]
fn branch_create_and_checkout() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init_repo(repo);
    commit_empty(repo);
    let g = git();
    let name = autopilot_branch_name("run-1.a_b").unwrap();
    assert_eq!(name, "autopilot/run-1.a_b");
    assert_eq!(g.run(repo, &["switch", "-c", &name]).unwrap().code, 0);
    let cur = g.run(repo, &["branch", "--show-current"]).unwrap();
    assert_eq!(cur.stdout.trim(), name);
    assert_eq!(g.run(repo, &["checkout", "main"]).unwrap().code, 0);
    let cur = g.run(repo, &["branch", "--show-current"]).unwrap();
    assert_eq!(cur.stdout.trim(), "main");
}

#[test]
fn status_and_diff_output_parse() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    init_repo(repo);
    std::fs::write(repo.join("a.txt"), "one\ntwo\n").unwrap();
    let g = git();
    assert_eq!(g.run(repo, &["add", "a.txt"]).unwrap().code, 0);
    assert_eq!(g.run(repo, &["commit", "-q", "-m", "a"]).unwrap().code, 0);
    std::fs::write(repo.join("a.txt"), "one\nthree\nfour\n").unwrap();
    std::fs::write(repo.join("new file.txt"), "n").unwrap();

    let st = g.run(repo, &["status", "--porcelain=v1", "-z"]).unwrap();
    assert_eq!(
        parse_porcelain_status(&st.stdout),
        vec![(" M".to_string(), "a.txt".to_string()), ("??".to_string(), "new file.txt".to_string())]
    );
    let diff = g.run(repo, &["diff", "--shortstat"]).unwrap();
    assert_eq!(parse_diff_stat(&diff.stdout), (1, 2, 1));
}

#[test]
fn missing_repo_is_nonzero_or_error() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("nope");
    match git().run(&missing, &["status"]) {
        Ok(out) => assert_ne!(out.code, 0),
        Err(_) => {}
    }
}

#[test]
fn slow_command_times_out() {
    let tmp = tempfile::tempdir().unwrap();
    init_repo(tmp.path());
    let g = ProcessGit::with_timeout(Duration::from_millis(500));
    let err = g.run(tmp.path(), &["-c", "alias.slow=!sleep 20", "slow"]).unwrap_err();
    assert_eq!(err.kind, PortErrorKind::Timeout);
}

#[test]
fn branch_name_validation() {
    assert_eq!(autopilot_branch_name("a").unwrap(), "autopilot/a");
    assert_eq!(autopilot_branch_name(&"a".repeat(64)).unwrap().len(), "autopilot/".len() + 64);
    for bad in ["", "../x", "a..b", "a/b", "a b", "a;b", "한글", ".x", "x.", "x.lock"] {
        assert!(autopilot_branch_name(bad).is_err(), "받아들이면 안 됨: {bad:?}");
    }
    assert!(autopilot_branch_name(&"a".repeat(65)).is_err());
}
