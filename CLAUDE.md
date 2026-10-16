# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 개요

Claude Board 는 Claude Code 세션 여러 개의 상태(승인 대기·작업 중·대기)를 한 화면에 모아 보여 주는 macOS 14+ 앱이다.
Claude Code hook 이 Python 스크립트를 실행해 세션 상태를 파일로 남기고, Swift 앱(보드·알림 패널)과 TUI 가 그 파일을 읽는다.
문서·코드 주석·커밋 메시지는 모두 한국어로 쓴다.

## 명령

Xcode 프로젝트·SwiftPM 이 없다. Swift 파일 하나가 실행 파일 하나이고 `swiftc` 로 바로 빌드한다. 테스트·린트 설정은 없다.

```sh
# 빠른 확인 (ClaudeBoard.swift 약 4초)
swiftc -typecheck ClaudeBoard.swift
swiftc -typecheck ClaudeNotifier.swift
python3 -m py_compile state.py notify.py hooks.py board-tui.py   # 만들어진 __pycache__ 는 지운다

# 로컬 빌드·설치 — 실제 환경을 덮어쓴다
./build-board.sh
#  → /Applications/Claude Board.app 을 다시 빌드하고(서명 인증서가 없으면 ad-hoc 서명)
#  → ~/.claude/notifier/ 에 claude-notifier 와 Python 스크립트를 복사한다 (지금 도는 hook 이 바로 새 스크립트를 쓴다)
#  → 떠 있는 보드는 다시 띄우지 않는다. 바꾼 앱을 보려면: pkill -x ClaudeBoard; open -a "Claude Board"

# 배포 (관리자) — CHANGELOG.md 맨 위에 "## <버전> — <날짜>" 항목을 쓰고 commit·push 한 뒤
packaging/release.sh <버전>   # make-dmg.sh 로 universal(arm64+x86_64) 빌드·서명 → gh release 생성
```

`release.sh` 는 CHANGELOG 의 `## <버전>` 항목이 없으면 빌드 전에 멈추고, 그 항목을 릴리스 설명으로 쓴다. `gh auth login` 과 서명 인증서(`packaging/make-cert.sh` 로 만드는 "Claude Board Signing")가 필요하다.

## 구조

### 데이터 흐름

```
Claude Code hook ──> state.py  ──> ~/.claude/notifier/sessions/<session_id>.json ──> ClaudeBoard (1초마다 읽음), board-tui.py
                 └─> notify.py ──> claude-notifier 패널을 분리 프로세스로 띄움 (Stop·Notification 때)
```

- `hooks.py` 의 `HOOKS` 표가 이벤트 11개를 `state.py <working|waiting|idle|ended|subagent>` / `notify.py <stop|notification>` 에 연결해 `~/.claude/settings.json` 에 넣는다.
  자기 hook 은 명령 문자열에 `.claude/notifier/` 가 들어간 것으로 알아보고, `packaging/install.sh` 도 같은 표시로 처음 설치인지 판단한다.
- hook 스크립트는 Claude Code 를 막거나 실패시키면 안 된다 — `main()` 전체를 `except Exception: pass` 로 감싸고, 패널은 `start_new_session=True` 로 띄우고 바로 끝난다.
- `state.py` 는 서브에이전트가 동시에 hook 을 돌리므로 `.<sid>.lock` 을 `flock` 으로 잡고 읽고-고쳐-쓴다. 서브에이전트 이벤트(`agent_id` 있음)는 세션 상태 대신 `subagents` 에만 남긴다.
- 스크립트는 자기 파일 옆의 `sessions/` 를 쓴다 — 저장소에서 바로 실행하면 저장소 안에 `sessions/` 가 생긴다.

### 파일로 주고받는 약속 (`~/.claude/notifier/`)

여러 프로그램이 파일만으로 상태를 나누므로, 파일 이름이나 JSON 필드를 바꾸면 읽는 쪽을 모두 고쳐야 한다.

| 파일 | 쓰는 쪽 | 읽는 쪽 |
|---|---|---|
| `sessions/<sid>.json` (`state`·`pid`·`tty`·`headless`·`cwd`·`topic`·`last_message`·`subagents`·`limit_hit`·`resume_cmd`·`transcript_path`·`model` …) | state.py | ClaudeBoard.swift, board-tui.py, notify.py(상태별 개수) |
| `sessions/<sid>.mute`, `mute-all`, `notify-position` | 보드 | notify.py |
| `sessions/<sid>.paused` | 보드 | state.py (새 요청이 오면 지움) |
| `context/<sid>.json` | 사용자의 상태줄 스크립트 (이 저장소에 없음) | 보드 (컨텍스트 사용량·effort) |
| `/tmp/.claude-usage-cache.json` | 보드·상태줄 스크립트가 함께 씀 | 보드 |

### 실행 파일

- `ClaudeBoard.swift` (약 5,200줄) — 보드 앱 전체. AppKit 창 위에 SwiftUI 화면을 얹었고 `// MARK:` 로 나뉜다.
  주요 부분: `Store`(세션 파일을 mtime 캐시로 1초마다 읽어 경로별 `Group` 으로 묶음), `UsageStore`(키체인의 OAuth 토큰으로 `api.anthropic.com/api/oauth/usage` 조회),
  `AccountStore`·`AutoSwitcher`(키체인 `Claude Code-credentials` 와 `~/.claude.json` 의 `oauthAccount` 를 `/usr/bin/security` 로 바꿔 끼워 계정 전환, 저장 계정은 키체인 서비스 `Claude Board-account`),
  `Updater`(최신 릴리스의 `VERSION` 비교 후 `install.sh` 실행), 터미널 포커스·일시중지·이어 열기(Ghostty·iTerm2·Terminal 에 AppleScript, `focusGhostty`·`ghosttySend` 등).
  창 크기에 따라 `BoardTier`(full·medium·mini)로 그린다. `$TMPDIR/claude-board.lock` 을 `flock` 해 한 번만 뜨고, 두 번째 실행은 `claude-board.show` 분산 알림만 보내고 끝난다.
- `ClaudeNotifier.swift` — 알림 패널. 인자는 파일 머리 주석 참고. 여러 패널이 동시에 뜨면 슬롯 파일로 자리를 나눈다.
- `board-tui.py` — 터미널판 보드(`claude-board-tui`).
- `AppIcon.swift` — 앱 아이콘 PNG(iconset 이름 규칙)를 인자로 준 폴더에 그린다. 결과 `AppIcon.icns` 는 저장소에 커밋되어 있다.
- `ClaudeStatus.swift` — 예전 메뉴 막대 앱. 어떤 빌드 스크립트도 빌드하지 않는다.

### 패키징과 배포

- Info.plist 는 `build-board.sh` 와 `packaging/make-dmg.sh` 안에 각각 heredoc 으로 있다. 키를 바꾸면 두 곳을 함께 고친다(버전은 make-dmg.sh 만 넣는다).
- `hooks.py` 와 `packaging/hooks.py` 는 같은 내용의 사본이다. build-board.sh 는 앞의 것, make-dmg.sh 는 뒤의 것을 앱에 넣으므로 둘을 함께 고친다.
- 설치 경로는 두 가지다: `packaging/install.sh`(curl 설치·업데이트, 보드의 업데이트 단추도 이것을 실행)와 dmg 안의 pkg(`pkg/scripts/postinstall` → `user-setup.sh`). 둘 다 사용자 설정은 앱 안의 `user-setup.sh` 가 맡는다.
- 저장소 `suhojang/claude-board` 와 릴리스 파일 이름 `ClaudeBoard.zip`·`install.sh`·`VERSION` 은 `release.sh`·`install.sh`·`ClaudeBoard.swift`(`releaseRepo`) 가 함께 쓴다 — 이름을 바꾸면 이미 설치된 앱의 업데이트가 깨진다.
- `install.sh` 는 서명 인증서의 SHA1(`CERT_SHA1`)을 고정해 확인한다. 인증서를 바꾸면 기존 사용자의 업데이트가 거부되고 Ghostty 제어 권한을 다시 묻는다.

## 저장소 관례

- `.gitignore` 는 `/*` 로 모두 빼고 `!/<이름>` 으로 올릴 파일만 고르는 방식이다. 최상위에 새 파일을 만들면 여기에 추가해야 추적된다. `dist/` 의 빌드 결과는 올리지 않는다.
- 사용자에게 보이는 기능이 바뀌면 `CHANGELOG.md`(바뀜·추가·고침), `README.md`, `packaging/README.txt`(설치 가이드 — 모든 기능의 자세한 설명)를 함께 고친다.
- 릴리스 커밋 메시지는 `<버전> 릴리스 — <요약>` 형식이다.

## Windows 판 (windows/)

Windows 판은 `windows/` 아래에만 있다(최상위 macOS 파일은 고치지 않는다). Rust 로 쓰고, 보드·알림 패널은 Tauri 2(Svelte 5 + TypeScript + Vite), hook·명령줄 도구는 순수 Rust 실행 파일이다.
`windows/docs/SPEC.md` 가 macOS 소스를 역분석한 단일 기준 명세다 — 동작을 바꾸거나 더할 때 먼저 여기에 맞춘다.

### 구조

- `crates/core`(라이브러리: 파일 약속·hook 상태 규칙·hook 등록·경로·Windows 프로세스 도구)와 `crates/hook`(`claude-board-hook.exe`: `state`·`notify`·`setup`·`uninstall`·`install-hooks`·`uninstall-hooks`·`tui`)은 `windows/Cargo.toml` 워크스페이스다.
- `app/` 은 프론트엔드(`package.json`·`src/`), `app/src-tauri/` 는 Tauri 백엔드(`ClaudeBoard.exe` — 인자 없음 = 보드, `--notify …` = 알림 패널 한 개)다. 백엔드는 워크스페이스 밖이라 **Cargo.lock 이 두 개**(`windows/Cargo.lock`, `windows/app/src-tauri/Cargo.lock`)다.
- `packaging/` — `install.ps1`(설치·업데이트, 보드의 업데이트 단추도 이것을 `-Version <v> -Update` 로 실행), `uninstall.ps1`, `nsis-hooks.nsh`(setup.exe 설치 후 `setup`, 제거 전 `uninstall`), `release.sh`(배포 준비 도우미).
- `ci/` — CI smoke 작업이 Windows 러너에서 돌리는 설치·화면 확인 스크립트(setup.exe·install.ps1 무인 설치·제거, 보드·알림 패널 스크린샷). Windows 실기 시험은 `crates/*/tests/windows_*.rs` 와 src-tauri 의 `#[cfg(windows)]` 시험이다.
- 파일 약속(`~/.claude/notifier/` 의 파일 이름·JSON 필드)은 macOS 와 같다. 한쪽에서 바꾸면 다른 쪽도 고친다.
- 실행 파일 이름 `ClaudeBoard.exe`·`claude-board-hook.exe`, 릴리스 자산 이름(`ClaudeBoard-windows-x64.zip`(+`.sha256`)·`install.ps1`·`VERSION`)을 바꾸면 이미 설치된 hook·업데이트가 깨진다.
- hook exe 는 Tauri sidecar(`externalBin`)로 setup.exe 에 묶인다 — 빌드 전에 `app/src-tauri/binaries/claude-board-hook-x86_64-pc-windows-msvc.exe` 로 복사해 두어야 한다(CI 가 한다).
- `install.ps1`·`uninstall.ps1` 은 Windows PowerShell 5.1 에서도 돌아야 한다(`??`·삼항·`&&` 금지). `irm | iex` 로 호출한 셸 안에서 돌므로 `exit` 를 쓰지 않고, BOM 없이 코드는 ASCII 로만 쓴다.

### 명령

```sh
cd windows && cargo test                                    # core·hook 단위 테스트 (macOS 에서도 돈다)
cd windows && cargo check --target x86_64-pc-windows-msvc   # Windows 전용 코드 컴파일 확인(링크 전까지). rustup target add x86_64-pc-windows-msvc 필요
cd windows/app && npm ci && npm run tauri dev               # 보드를 띄워 화면 확인 (macOS 에서는 가짜 세션 파일로)
```

### 버전과 배포

- 버전은 `windows/VERSION` 한 곳에 둔다(macOS 와 같은 번호, 첫 판 2.12). Tauri 에는 CI 가 semver(2.12 → 2.12.0)로 넘긴다.
- 배포: `windows/CHANGELOG.md` 맨 위에 `## <버전> — <날짜>` 항목 → `windows/VERSION` 올리기(`windows/packaging/release.sh <버전>` 이 항목 확인 후 바꾸고, README·설치 가이드의 setup.exe·릴리스 링크도 새 버전으로 바꿔 준다) → main 에 push.
  `.github/workflows/windows.yml` 이 빌드·테스트하고 smoke 작업으로 실제 설치를 확인한 뒤(실패하면 릴리스하지 않음), 태그 `windows-v<버전>` 이 아직 없으면 그 항목을 설명으로 릴리스를 만든다(항목이 없으면 실패).
  push 전에 브랜치에서 `gh workflow run windows.yml --ref <브랜치> -f dry_run=true` 로 릴리스 작업의 모든 확인을 돌려 볼 수 있다(아무것도 만들지 않고 gh 명령만 보여 준다). `dry_run=false` 는 main 에서만 실제 릴리스를 만든다.
- **Windows 릴리스는 절대 Latest 로 표시하지 않는다**(`--latest=false`). 설치된 macOS 앱이 `releases/latest/download/...` 를 읽기 때문에 Windows 릴리스가 Latest 가 되면 macOS 업데이트가 깨진다.
  그래서 Windows 쪽 코드·스크립트는 `/releases/latest` 를 쓰지 않고 릴리스 목록에서 `windows-v` 태그를 고른다. macOS 의 `packaging/release.sh` 는 최상위 `CHANGELOG.md` 만 본다.
- 사용자에게 보이는 기능이 바뀌면 `windows/CHANGELOG.md`·`windows/README.md`·`windows/docs/설치 가이드.md` 를 함께 고친다.
