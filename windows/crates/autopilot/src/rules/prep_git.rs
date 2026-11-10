//! (청크 PrepGit 1/2 전반만 이식됨 — 이 첫 줄은 `tests/ported.rs` 가 「구현됐는데 PORTED.json 에 없는 규칙」으로 잡지 않게 하는 표시다. R-PrepGit-b 가 후반을 옮긴 뒤 이 줄을 지우고 PORTED.json 항목을 넣는다)
//! 출발 준비·지난 실행 이어 가기에서 보드가 git 을 읽기 전의 git 자리 확인. 원본: `Sources/AutopilotRules/PrepGit.swift`.
//! 원본 머리 주석의 요지: 고른 경로가 모델이 쓸 수 있던 작업 트리 안의 중첩 git 이면 그 git 폴더 설정의 clean filter 를 보드의 git 이
//! 샌드박스 밖에서 돌린다. 그래서 git 을 부르기 전에 파일 시스템으로만 저장소 뿌리·git 폴더를 찾고(.git 바로 가기·HEAD 가 보통 파일이 아닌 꼴·
//! gitdir/commondir 되짚기), 중첩 git·임시 폴더·떨어진 git 폴더를 막고, git 설정(core.fsmonitor·filter·include)과 하위 모듈을 본다.
//! 파일 시스템·git 은 포트(`PrepGitWorld`)로만 본다 — 진짜 어댑터는 `adapters/prep_git_world.rs`, 가짜는 시험.
//!
//! 이식된 선언(원본 순서, 원본 1~99줄): `GitEntry`(APGitEntry) · `ConfigEntry`(APConfigEntry) · `PrepGitWorld`(APPrepGitWorld) ·
//! `PrepGit`(APPrepGit, `pin`) · `lexical` · `short_path`(apPrepShortPath) · `fsmonitor_is_bool` · `PREP_GIT_READ_ENV`(apPrepGitReadEnv) ·
//! `prep_git_env`(apPrepGitEnv) · `lfs_filter`(apLFSFilter).
//! 남은 선언(R-PrepGit-b, 원본 100줄 「// 유스케이스: 경로 하나…」 주석부터): 비공개 `parent`(원본 70~73줄, `board_git::parent_path` 가
//! `pub(crate)` 가 되거나 여기서 뿌리 판정까지 합쳐 옮긴다) · `apPrepGitCheck` · `apPrepGitBlockText`.
//!
//! Windows 판단(원본에 없는 것, 모두 시험으로 지킨다):
//! - 환경: 윈도는 환경 변수 이름의 대소문자를 가리지 않으므로 `git_config_parameters` 같은 소문자 꼴도 `GIT_*` 로 보고 뺀다(그대로 두면
//!   git 이 읽는다 — macOS 보다 느슨해지는 구멍).
//! - `lexical`: `\`·드라이브(`C:\`)·`\\?\`·`\\.\`·UNC(`\\서버\공유`)·뿌리 없는 `\x` 꼴은 `/` 구분자 한 가지로 맞추고 `\\?\` 접두는 벗긴다.
//!   뒤에 붙은 점·공백(윈도가 무시하는 꼴)은 뗀다. 같은 자리가 같은 글이 되도록 한쪽(막는 쪽)으로만 기운다. 견줄 때의 소문자 접기는 부르는 쪽 몫(원본과 같다).
//! - `short_path`: 홈이 윈도 꼴(드라이브·UNC)이면 접두를 대소문자 가리지 않고, 구분자는 `/`·`\` 둘 다 받는다. 결과는 `~` 뒤에 실제 구분자를 그대로 둔다. 보이는 글일 뿐 판정에 쓰지 않는다.

use std::collections::BTreeMap;

/// `<폴더>/.git` 의 꼴(따라가지 않고 본 것). 원본: `APGitEntry`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitEntry {
    /// `.git` 폴더
    Dir,
    /// `.git` 파일 — "gitdir:" 뒤 글(그 꼴이 아니거나 못 읽으면 None)
    File(Option<String>),
    /// 바로 가기(심볼릭 링크·Windows 정션·재분석 지점)
    Link,
    /// FIFO·장치 등
    Other,
}

/// git 설정 한 줄(`git config --null --list` 꼴: 키는 git 이 소문자로 준 것, 값 없는 키는 None). 원본: `APConfigEntry`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
    pub key: String,
    pub value: Option<String>,
}

impl ConfigEntry {
    pub fn new(key: &str, value: Option<&str>) -> Self {
        ConfigEntry { key: key.to_string(), value: value.map(str::to_string) }
    }
}

/// 파일 시스템·git 을 보는 포트. 원본: `APPrepGitWorld`
pub trait PrepGitWorld {
    /// `<dir>/.git` 의 꼴(lstat). 없으면 None
    fn entry(&self, dir: &str) -> Option<GitEntry>;
    /// 실제 경로(realpath). 없으면 None
    fn real(&self, path: &str) -> Option<String>;
    /// 작은 보통 파일의 글(바로 가기·FIFO·큰 파일은 None)
    fn text(&self, path: &str) -> Option<String>;
    /// 보통 파일인가 · 폴더인가(lstat, 바로 가기는 아님)
    fn is_file(&self, path: &str) -> bool;
    fn is_dir(&self, path: &str) -> bool;
    /// "장치:inode"(lstat). Windows 는 "볼륨 일련번호:파일 인덱스"
    fn stamp(&self, path: &str) -> Option<String>;
    /// `git config --file <파일> --no-includes --null --list`(저장소를 찾지 않는 자리에서). 파일이 없으면 `Some(vec![])`, 보통 파일이 아니거나 읽지 못하면 None
    fn config(&self, file: &str) -> Option<Vec<ConfigEntry>>;
    /// 박는 env(pin: GIT_DIR·GIT_COMMON_DIR·GIT_WORK_TREE 와 `PREP_GIT_READ_ENV`)로 색인의 gitlink(`ls-files -s -z`)와
    /// .gitmodules(작업 트리·색인·HEAD) 를 본다. 반환은 `(gitlinks, gitmodules)`(원본의 이름 붙은 튜플). 색인을 읽지 못하면 None
    fn submodules(&self, top: &str, pin: &[String]) -> Option<(Vec<String>, bool)>;
}

/// 경로 하나의 저장소와 막을 까닭. 원본: `APPrepGit`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PrepGit {
    /// 저장소 뿌리(파일 시스템으로 찾은 실제 경로). None 이면 git 저장소가 아님(위로 뿌리까지 .git 없음)
    pub top: Option<String>,
    /// 그 자리의 git 폴더(따로 작업 트리면 .git/worktrees/<이름>)
    pub git_dir: Option<String>,
    /// 공통 git 폴더(가지·객체·config)
    pub common_dir: Option<String>,
    /// 막은 까닭 — 있으면 이 저장소에서 git 을 더 돌리지 않는다
    pub block: Option<String>,
}

impl PrepGit {
    /// 통과한 저장소의 읽기(지금 가지·가지 목록·status·rev-list)에 박는 env — git 이 위로 저장소를 스스로 찾지 않게, 원격으로 객체를 받지 않게(`PREP_GIT_READ_ENV`).
    /// 공통 git 폴더도 확인한 자리로 박는다(GIT_COMMON_DIR 가 있으면 git 은 git 폴더의 commondir 파일을 읽지 않는다 — 확인 뒤에 commondir 를 심어도 따르지 않게).
    /// 막혔거나 뿌리·git 폴더를 모르면 빈 목록. 원본: `APPrepGit.pin`
    pub fn pin(&self) -> Vec<String> {
        let (None, Some(t), Some(g)) = (&self.block, &self.top, &self.git_dir) else { return Vec::new() };
        let common = self.common_dir.as_ref().unwrap_or(g);
        let mut env = vec![format!("GIT_DIR={g}"), format!("GIT_COMMON_DIR={common}"), format!("GIT_WORK_TREE={t}")];
        env.extend(PREP_GIT_READ_ENV.iter().map(|s| s.to_string()));
        env
    }
}

fn is_sep(c: char) -> bool {
    c == '/' || c == '\\'
}

fn is_drive(b: &[u8]) -> bool {
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b.len() == 2 || b[2] == b'/' || b[2] == b'\\')
}

/// 윈도 꼴 경로인가 — 드라이브(`C:`·`C:\`·`C:/`)나 `\`로 시작(`\\서버`·`\\?\`·`\x`). `/` 로만 쓴 `//a` 는 POSIX 로 본다(원본은 `//a` 를 `/a` 로 접는다).
fn is_windows_style(p: &str) -> bool {
    is_drive(p.as_bytes()) || p.starts_with('\\')
}

/// 경로의 `.`·`..`·겹친 구분자·끝 구분자를 글자만으로 정리한 꼴. 원본: `lexical`(= `URL(fileURLWithPath:).standardizedFileURL.path`).
/// 원본이 `/private` 접두를 실제 파일 시스템을 보고 벗기는 일은 하지 않는다(그 일은 `real` 몫이고 원본 범위 목록이 둘 다 든다).
/// 원본은 상대 경로를 작업 폴더 기준으로 풀지만 부르는 쪽은 늘 절대 경로를 준다 — 여기서는 상대 경로를 그대로 정리만 한다(`..` 는 앞에서 버린다).
/// Windows 꼴은 구분자를 `/` 한 가지로(`C:\a\..\b` → `C:/b`, `\\?\C:\a` → `C:/a`, `\\srv\sh\x` → `//srv/sh/x`, `\\?\UNC\srv\sh` → `//srv/sh/`),
/// 성분 끝의 점·공백을 떼고(윈도는 `a.` 를 `a` 로 본다), 뿌리(`C:/`·`//서버/공유/`·`/`) 위로는 올라가지 않는다.
pub fn lexical(p: &str) -> String {
    let win = is_windows_style(p);
    let (prefix, rooted, rest): (String, bool, &str) = if win { split_windows_prefix(p) } else { (String::new(), p.starts_with('/'), p) };
    let mut stack: Vec<&str> = Vec::new();
    for c in rest.split(|ch: char| if win { is_sep(ch) } else { ch == '/' }) {
        let c = if win && c != "." && c != ".." { c.trim_end_matches(['.', ' ']) } else { c };
        match c {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            _ => stack.push(c),
        }
    }
    let body = stack.join("/");
    if rooted {
        format!("{prefix}/{body}")
    } else {
        format!("{prefix}{body}")
    }
}

/// Windows 꼴 경로를 (접두, 뿌리가 있나, 접두 뒤 나머지)로 가른다. 접두는 `C:`·`//서버/공유`·빈 글이고 구분자는 `/` 로 맞춘다.
fn split_windows_prefix(p: &str) -> (String, bool, &str) {
    // `\\?\`·`\\.\` — 뒤가 `UNC\서버\공유\…` 이면 UNC, 아니면 드라이브(또는 그 밖의 장치)로 읽는다
    for ns in [r"\\?\", r"\\.\", "//?/", "//./"] {
        if let Some(rest) = p.strip_prefix(ns) {
            if rest.get(..3).is_some_and(|h| h.eq_ignore_ascii_case("unc")) && rest[3..].starts_with(is_sep) {
                return split_unc(&rest[4..]);
            }
            // 접두 뒤는 늘 절대 자리다(`\\?\한글\x` 처럼 드라이브가 없어도)
            let (pre, rooted, r) = split_drive_or_root(rest);
            let rooted = rooted || pre.is_empty();
            return (pre, rooted, r);
        }
    }
    if let Some(after) = p.strip_prefix(r"\\") {
        return split_unc(after);
    }
    split_drive_or_root(p)
}

/// `서버\공유\나머지` → (`//서버/공유`, true, 나머지). 공유가 없으면 (`//서버`, true, "")
fn split_unc(after: &str) -> (String, bool, &str) {
    let mut it = after.splitn(3, is_sep);
    let server = it.next().unwrap_or("");
    let share = it.next().unwrap_or("");
    let rest = it.next().unwrap_or("");
    let prefix = if share.is_empty() { format!("//{server}") } else { format!("//{server}/{share}") };
    (prefix, true, rest)
}

fn split_drive_or_root(p: &str) -> (String, bool, &str) {
    if is_drive(p.as_bytes()) {
        let rooted = p.len() > 2;
        return (p[..2].to_string(), rooted, &p[2..]);
    }
    (String::new(), p.starts_with(is_sep), p)
}

/// 보이는 글의 홈 줄임: `<홈>/…` 이면 `~/…`. 홈이 윈도 꼴이면 접두는 대소문자를 가리지 않고 구분자는 `/`·`\` 둘 다 받으며 `~` 뒤에 실제 구분자를 둔다.
/// 원본: `apPrepShortPath`
pub fn short_path(p: &str, home: &str) -> String {
    if is_windows_style(home) {
        if let Some(head) = p.get(..home.len()) {
            let same = head.len() == home.len()
                && head.bytes().zip(home.bytes()).all(|(a, b)| a.eq_ignore_ascii_case(&b) || (is_sep(a as char) && is_sep(b as char)));
            if same {
                let rest = &p[home.len()..];
                if rest.starts_with(is_sep) {
                    return format!("~{rest}");
                }
            }
        }
        return p.to_string();
    }
    match p.strip_prefix(home).and_then(|r| r.strip_prefix('/')) {
        Some(rest) => format!("~/{rest}"),
        None => p.to_string(),
    }
}

/// 명령이 아닌 `core.fsmonitor` 값(끔·켬, git 의 불리언 꼴). 켬(true)은 git 이 든 내장 감시이고, 그 밖의 글은 돌릴 명령(hook 경로)이다.
/// 값 없음(None)은 불리언. 앞뒤를 자르는 글자는 원본 `.whitespaces`(Unicode Zs 와 탭)와 같다 — Rust 의 `trim()` 은 줄바꿈도 잘라
/// `"false\n"` 을 불리언으로 받아 원본보다 느슨해진다. 원본: `fsmonitorIsBool`
pub fn fsmonitor_is_bool(v: Option<&str>) -> bool {
    let Some(v) = v else { return true };
    let t = v.trim_matches(is_swift_whitespace).to_lowercase();
    ["", "true", "false", "yes", "no", "on", "off", "1", "0"].contains(&t.as_str())
}

/// Swift `CharacterSet.whitespaces` — 일반 범주 Zs 와 탭
fn is_swift_whitespace(c: char) -> bool {
    matches!(c, '\t' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// 출발 준비의 git 읽기는 부분 복제(promisor) 저장소에서 빠진 객체를 받으려고 원격 명령(remote.*.uploadpack·core.sshCommand)을 부르지 않게
/// 늦은 받기를 끄고 모든 전송 프로토콜을 막는다(GIT_CONFIG_COUNT 로 명령줄 설정과 같은 무게). 원본: `apPrepGitReadEnv`
pub const PREP_GIT_READ_ENV: [&str; 4] = ["GIT_NO_LAZY_FETCH=1", "GIT_CONFIG_COUNT=1", "GIT_CONFIG_KEY_0=protocol.allow", "GIT_CONFIG_VALUE_0=never"];

/// 출발 준비 읽기의 환경 — 보드가 물려받은 `GIT_*`(GIT_CONFIG_PARAMETERS·GIT_CONFIG_COUNT/KEY/VALUE·GIT_SSH*·GIT_ALLOW_PROTOCOL·GIT_DIR·GIT_INDEX_FILE 등)는
/// 모두 빼고, 시스템·전역 설정·attributes·replace 를 읽지 않는 보호(보드 `apGitEnv` 와 같은 넷)와 박는 값(`extra`: "이름=값")만 넣는다.
/// 이름이 `GIT_` 로 시작하는지는 대소문자를 가리지 않는다(윈도 환경 변수 이름은 대소문자를 가리지 않는다 — 원본은 대문자만 뺀다).
/// `extra` 의 칸은 첫 `=` 에서 가르고 `=` 가 없으면 버린다. 원본: `apPrepGitEnv`
pub fn prep_git_env(inherited: &BTreeMap<String, String>, extra: &[String]) -> BTreeMap<String, String> {
    let mut env: BTreeMap<String, String> =
        inherited.iter().filter(|(k, _)| !k.to_ascii_uppercase().starts_with("GIT_")).map(|(k, v)| (k.clone(), v.clone())).collect();
    for (k, v) in [("GIT_CONFIG_NOSYSTEM", "1"), ("GIT_CONFIG_GLOBAL", "/dev/null"), ("GIT_ATTR_NOSYSTEM", "1"), ("GIT_NO_REPLACE_OBJECTS", "1")] {
        env.insert(k.to_string(), v.to_string());
    }
    for s in extra {
        if let Some((k, v)) = s.split_once('=') {
            env.insert(k.to_string(), v.to_string());
        }
    }
    env
}

/// git-lfs 가 저장소에 쓰는 표준 filter 글(`git lfs install --local`) — 이 글 그대로일 때만 filter 명령을 받는다. 키는 소문자. 원본: `apLFSFilter`(사전)
/// 원본은 `apLFSFilter[k] == e.value` 로 견주므로 표에 없는 키와 값 없는 설정(None)도 같다고 나온다 — 부르는 쪽은 `lfs_filter(k) == value` 로 그대로 견준다.
pub fn lfs_filter(key: &str) -> Option<&'static str> {
    match key {
        "filter.lfs.clean" => Some("git-lfs clean -- %f"),
        "filter.lfs.smudge" => Some("git-lfs smudge -- %f"),
        "filter.lfs.process" => Some("git-lfs filter-process"),
        _ => None,
    }
}
