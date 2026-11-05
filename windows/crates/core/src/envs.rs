//! 실행 환경 목록 — 내장 터미널 새 탭의 셸과 자율주행 실행 환경이 함께 쓰는 하나의 목록.
//! id 는 터미널 독(`terminal.rs DOCK`)과 같은 글자를 쓰고 WSL 만 더한다.
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvKind {
    Wsl,
    Pwsh,
    PowerShell,
    Cmd,
    GitBash,
}

impl EnvKind {
    pub const ALL: [EnvKind; 5] = [EnvKind::Wsl, EnvKind::Pwsh, EnvKind::PowerShell, EnvKind::Cmd, EnvKind::GitBash];

    pub fn id(self) -> &'static str {
        match self {
            EnvKind::Wsl => "wsl",
            EnvKind::Pwsh => "pwsh",
            EnvKind::PowerShell => "powershell",
            EnvKind::Cmd => "cmd",
            EnvKind::GitBash => "git-bash",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.id() == id)
    }

    pub fn label(self) -> &'static str {
        match self {
            EnvKind::Wsl => "WSL",
            EnvKind::Pwsh => "PowerShell 7",
            EnvKind::PowerShell => "Windows PowerShell",
            EnvKind::Cmd => "명령 프롬프트",
            EnvKind::GitBash => "Git Bash",
        }
    }

    /// Claude Code 샌드박스를 걸 수 있는 환경인가 — 공식 문서상 WSL2 뿐(네이티브 Windows 는 샌드박스 없이 돈다).
    pub fn sandbox_capable(self) -> bool {
        matches!(self, EnvKind::Wsl)
    }
}

/// 고른 환경. 키 꼴: `wsl:<배포판>` · `wsl`(기본 배포판) · `pwsh` · `powershell` · `cmd` · `git-bash`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnvChoice {
    pub kind: EnvKind,
    pub distro: Option<String>,
}

impl EnvChoice {
    pub fn key(&self) -> String {
        match (self.kind, &self.distro) {
            (EnvKind::Wsl, Some(d)) => format!("wsl:{d}"),
            (k, _) => k.id().to_string(),
        }
    }

    /// 배포판 이름은 `[A-Za-z0-9._-]{1,64}` 만 받는다(인자로 넘기므로). 그 밖은 None.
    pub fn parse(key: &str) -> Option<Self> {
        if let Some(d) = key.strip_prefix("wsl:") {
            let ok = !d.is_empty()
                && d.len() <= 64
                && d.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
            return ok.then(|| EnvChoice { kind: EnvKind::Wsl, distro: Some(d.to_string()) });
        }
        EnvKind::from_id(key).map(|kind| EnvChoice { kind, distro: None })
    }
}

/// 이 PC 에 있는 환경 하나. `program` 은 띄울 실행 파일
/// (Git Bash 는 mintty 가 아니라 `Git\bin\bash.exe`, WSL 은 `%SystemRoot%\System32\wsl.exe`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvEntry {
    pub choice: EnvChoice,
    pub label: String,
    pub program: PathBuf,
}

/// `wsl.exe -l -q` 출력(UTF-16LE, BOM 이 있을 수도 없을 수도) → 배포판 이름.
/// 빈 줄, `docker-desktop` 으로 시작하는 이름, 위 이름 규칙에 어긋나는 이름은 뺀다.
pub fn parse_wsl_list(_raw: &[u8]) -> Vec<String> {
    todo!("E0")
}

/// 있는 환경을 `EnvKind::ALL` 차례로 돌려준다(Windows 가 아니면 빈 목록).
/// WSL 은 wsl.exe 가 있고 `-l -q` 가 5초 안에 종료 코드 0 과 배포판을 하나 이상 줄 때만 넣는다(배포판마다 한 줄).
/// 기능이 꺼진 PC 의 wsl.exe 는 설치 안내만 내므로, 파일이 있는지만으로는 판정하지 않는다.
pub fn detect() -> Vec<EnvEntry> {
    todo!("E0")
}
