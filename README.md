# Claude Board

Claude Code 세션 상태를 한눈에 보여 주는 macOS 보드 앱과, 응답이 끝나거나 승인이 필요할 때 화면 가운데에 뜨는 알림 패널입니다.

- 작업 경로별로 세션을 묶어 작업 중 · 승인 대기 · 대기 · 종료 상태를 보여 줍니다.
- 세션을 더블 클릭하면 그 세션의 Ghostty 탭으로 이동하고, 종료된 세션은 `claude --resume` 으로 이어 엽니다.
- 5시간 · 주간 사용 한도와 계정 전환을 보드에서 바로 다룹니다.

자세한 사용법은 [설치 가이드](packaging/README.txt), 버전별 변경은 [CHANGELOG.md](CHANGELOG.md) 를 보세요.

## 필요한 환경

- macOS 14(Sonoma) 이상 — Apple Silicon · Intel
- Claude Code
- python3 (`python3 --version` 이 나오면 됩니다. 없으면 `xcode-select --install`)
- 세션 탭 이동 · 이어 열기는 Ghostty 1.3 이상에서만 동작합니다. 보드와 알림은 다른 터미널에서도 동작합니다.

## 설치

```sh
curl -fsSL https://github.com/suhojang/claude-board/releases/latest/download/install.sh | bash
```

- 처음 설치할 때 `~/.claude/settings.json` 에 hook 11개와 환경변수 `CLAUDE_CODE_DISABLE_TERMINAL_TITLE=1` 을 등록할지 묻습니다.
  원본은 `settings.json.bak-claude-board-<날짜-시각>` 으로 백업하고, 다른 hook · 설정은 그대로 둡니다.
- 예전에 dmg 로 설치한 앱을 바꿀 때만 관리자 비밀번호 창이 한 번 뜹니다.
- 설치가 끝나면 보드가 열립니다. 이미 실행 중인 Claude Code 세션은 다시 시작해야 hook 이 적용됩니다.

설치하면 `/Applications/Claude Board.app`, `~/.claude/notifier/` 의 알림 패널 · hook 스크립트, 로그인 자동 실행(LaunchAgent),
`~/.local/bin` 의 `claude-board` · `claude-board-tui` 명령이 생깁니다. Ghostty 가 있으면 Ghostty 를 열 때 보드를 함께 띄우는 줄을 `~/.zshrc` 에 넣습니다.

## 업데이트

새 버전이 나오면 보드 윗줄 아래에 알림이 뜹니다. **업데이트** 를 누르면 새 버전으로 바꿔 끼우고 보드가 다시 열립니다.
위 설치 명령을 다시 실행해도 같습니다. hook 등록은 다시 묻지 않습니다.

## 제거

```sh
curl -fsSL https://raw.githubusercontent.com/suhojang/claude-board/main/packaging/Uninstall.command | bash
```

앱, 설치된 파일, settings.json 의 hook 과 환경변수, 자동 실행 등록, `~/.zshrc` 에 넣은 줄, 계정 전환용으로 저장한 로그인 정보를 지웁니다.
`/Applications` 의 앱을 지울 때 관리자 비밀번호를 묻습니다.

## 배포 (관리자)

1. [CHANGELOG.md](CHANGELOG.md) 맨 위에 새 버전 항목을 쓰고 commit · push 합니다.
2. `packaging/release.sh <버전>` — 앱을 빌드 · 서명하고 GitHub 릴리스를 만듭니다. 릴리스 설명은 CHANGELOG 의 그 버전 항목입니다.

서명에는 `packaging/make-cert.sh` 로 만든 개인 인증서(Claude Board Signing)가 필요합니다. 인증서를 바꾸면 사용자 Mac 에서 Ghostty 제어 권한을 다시 묻고, `install.sh` 가 설치를 거부합니다.
