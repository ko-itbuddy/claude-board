# 내장 터미널 Windows 판 작업 목록

2026-10-18 정리. macOS 판(브랜치 `feature/embedded-terminal`, 1~7단계와 추가 요청)이 끝난 상태에서, Windows 판을 같은 동작으로 만들기 위한 일을 단계별로 모았다.
근거 문서: [Windows 계획.md](Windows%20계획.md)(1부 결정별 점검이 우선, 2부 첫 계획), [설계.md](설계.md) 7장(Windows 결정)과 11장(macOS 구현 메모·실측), `windows/docs/SPEC.md`(Windows 기준 명세), CLAUDE.md 의 "Windows 판" 절.

## 0. 먼저 알아 둘 것

- **이 Mac 에서는 Windows 를 직접 띄워 볼 수 없다.** 확인 수단은 `cargo test`(macOS 에서도 돔), `cargo check --target x86_64-pc-windows-msvc`, GitHub Actions 의 Windows 러너(smoke 작업이 설치·화면 스크린샷까지 찍음, `gh workflow run windows.yml --ref <브랜치> -f dry_run=true`), 그리고 사용자의 실제 Windows PC(한글 IME·ConPTY 키 등, 아래 7장).
- 작업 브랜치: `feature/embedded-terminal` 에서 이어 가거나, 여기서 `feature/windows-terminal` 을 새로 딴다(사용자 결정). main 병합은 사용자가 따로 요청할 때만.
- Windows 릴리스는 **절대 Latest 로 표시하지 않는다**(`--latest=false`). macOS 업데이트가 `releases/latest` 를 읽는다.
- 파일 약속(`~/.claude/notifier/` 파일 이름·JSON 필드)은 macOS 와 같다. 바꾸지 않는다.
- 사용자에게 보이는 글(화면 문자열·windows/README.md·windows/CHANGELOG.md·설치 가이드)에는 긴 줄표를 쓰지 않는다(2026-10-17 사용자 결정, CLAUDE.md 저장소 관례).

## 1. 계획 이후 macOS 에서 정해지거나 바뀐 것 (Windows 도 맞출 것)

Windows 계획.md 는 2026-10-17 에 쓰였다. 그 뒤 macOS 구현에서 아래가 정해지거나 바뀌었으므로 SPEC.md 에 먼저 반영한다.

| 항목 | macOS 결정·구현 | Windows 에 맞출 것 |
|---|---|---|
| 탭 실행 방식 | **탭은 늘 로그인 셸이고 claude 명령은 셸에 입력해 실행**(2026-10-18). `/exit` 하면 같은 탭에 대화가 남고 셸 프롬프트로 돌아온다. [다시 열기][셸 열기] 막대 없음. 셸에서 exit 하면 탭 닫힘. 중단은 claude 만 끝내고 셸은 남김 | 계획의 "셸 없이 바로 띄우기"·"[셸 열기]는 pwsh → powershell → cmd" 를 바꾼다: 탭 셸을 pwsh → powershell → cmd 순으로 고르고, 셸이 입력을 받을 수 있을 때 명령을 넣는다. 탭 소속 판정(같은 ConPTY 콘솔)과 "셸이 프롬프트에 있는가" 판정을 Windows 방식으로 |
| 보드 위치 | 윈도우 메뉴 "보드 위치: 오른쪽(기본)/왼쪽". 왼쪽이면 배치·접기·미니·처음 크기로 붙이기·앱 독이 모두 왼쪽 기준 | 같은 설정과 메뉴. 단축키 Ctrl+Alt+B 의 붙이는 쪽도 따름 |
| 펼치기·접기 애니메이션 | 메뉴로 켜고 끔, 기본 끔, 켜면 0.55초. 움직이는 동안 터미널 크기 고정(pty 크기 변화 0), 그동안 뜨는 창·확인 창은 끝난 뒤 | 같은 설정. xterm.js fit 을 애니메이션 끝에 한 번만 |
| ⋯ 메뉴 | 보드 모양 패널(세션 이름·상태·칩·폴더, 항목마다 아이콘·제목·설명, 세션 중단은 빨간색 따로, ↑↓·Return·Esc) | Svelte 로 같은 패널 |
| 수명 | 탭 목록을 바뀔 때마다 저장, 다시 뜨면 되살리고 이어 엶(보드 밖 pid 가 쥔 세션은 5초 기다린 뒤 뺌, 끝난 탭은 저장 안 함, 셸만 남은 탭은 셸 탭으로), 업데이트·종료 전 확인 창(보드 탭 세션만), 끊긴 작업 탭의 주 세션에 `.paused` | 같은 동작. 계획 12장의 열린 질문 ".paused 를 남길지" 는 **남긴다**로 정해짐(macOS 와 같게) |
| md-render | 앱에 싣고, 보드가 시작할 때 **사용자 폴더 `~/.claude/notifier/plugins/md-render/` 로 옮겨** 그 경로를 넘김(대화형 claude 가 플러그인 폴더에 `tsconfig.json`·`.claude-plugin/types/` 를 씀). 보드 탭 claude 는 모두 켬, 새 세션 창에서 끈 세션만 기억(`mdRenderOff`) | 계획의 "앱 폴더 `plugins\md-render` 를 그대로 넘김" 을 바꾼다: `%USERPROFILE%\.claude\notifier\plugins\md-render\` 로 옮겨 넘김(설치 폴더를 업데이트가 바꿔 끼워도 경로 유지). 옛 md-render `--plugin-dir`(값이 `plugins\md-render` 로 끝남)은 빼고 한 번만 |
| 끝난 세션 이어 열기 | 보드 탭에서. 그 세션이 돌던 탭이 셸 프롬프트로 남아 있으면 그 탭에 명령 입력 | 같게 |
| 이어 열기 명령 | `state.py` carry_args 가 같은 플래그·같은 (이름, 값) 을 한 번만 넘김(bypass 모드로 이어 열 때 Claude Code 가 `--allow-dangerously-skip-permissions` 를 하나 더 붙여 쌓이던 것) | `windows/crates/core/src/state.rs` 의 `carry_args` 도 같은 고침 + 단위 시험 |
| 버전 | **3.0.0, 시맨틱 버전**(2026-10-18) | `windows/VERSION` 을 3.0.0 으로(CI 는 x.y.z 를 그대로 받음). `windows/CHANGELOG.md` 머리 형식을 줄표 없는 `## <버전> (<날짜>)` 로 바꿀지 확인하고, 바꾸면 `windows/packaging/release.sh`·`windows.yml` 의 항목 찾기를 함께 맞춤 |
| 제3자 고지 | macOS 는 `packaging/THIRD_PARTY_NOTICES.txt` 를 앱에 실음 | xterm.js·애드온·ConPTY(sideload 시)·md-render 고지를 Windows 앱에 |

## 2. 단계별 작업

단계마다 빌드·시험하고 커밋한다. 확인 수단은 각 단계 끝에 적었다.

0. **SPEC 고치기(문서만)**: `windows/docs/SPEC.md` 에 1장 표와 Windows 계획.md 1부 결정을 반영한다. 계획 2부에서 바뀐 곳(Job 안 씀, 탭 실행 방식, md-render 경로)도 고친다. 확인: 사용자 검토.
1. **빌드·의존**: xterm.js 6.0.0(+WebGL·fit·글자 폭 애드온, 정확한 버전 고정, package-lock), windows-sys 로 쓴 얇은 ConPTY 래퍼(Tauri 백엔드), sideload ConPTY 를 실을지에 따라 `conpty\` 폴더. 확인: `npm ci`, `cargo check --target x86_64-pc-windows-msvc`, CI 빌드.
2. **터미널 영역**: 보드 창 안 터미널(펼치기·접기 단추·Ctrl+Alt+T·메뉴), 경계 끌기와 폭 기억, 탭 줄, 보드 위치, 애니메이션, 숨김 중 출력 유지, 한글 IME 고침 지점(`attachCustomKeyEventHandler`, #6065). 확인: CI 스크린샷, 실제 PC(7장).
3. **세션 연결**: 새 세션 창(작업 폴더·명령 기본값 `claude --dangerously-skip-permissions`·꾸미기 체크), 경로 카드 +, 셸 탭 + 명령 입력, `--session-id`, 세션 표지 환경 변수 지우기, `CreateEnvironmentBlock` 환경, 짝짓기(같은 ConPTY 콘솔), 칩, 클릭 흐름(보드 탭·다른 터미널·백그라운드·끝난 세션), 알림 패널 "세션으로 이동". 확인: 단위 시험, CI.
4. **조작**: 탭에 직접 입력(일시중지 Esc, 재개 문구, 중단은 claude 만), 백그라운드에는 신호 안 보냄·메뉴 없음, 보드로 옮기기(Windows Terminal 등 다른 터미널 세션을 /exit 하고 보드 탭에서 이어 열기, 가능한 범위 확인), ⋯ 패널. 확인: 실제 PC.
5. **수명**: 탭 목록 저장·되살리기, 옛 탭 pid 를 최대 5초 기다림, 업데이트·종료 확인 창, `.paused`, 보드가 꺼질 때 Job 없이 콘솔 끊김에 맡김(보드 쪽 파이프 끝은 상속 불가). 확인: CI(가능한 범위), 실제 PC.
6. **md-render**: Tauri `bundle.resources` 에 플러그인 파일, 시작할 때 사용자 폴더로 옮기기, `--plugin-dir` 규칙, 끈 세션 기억, **install.ps1 이 하위 폴더(`plugins\md-render\`·`conpty\`)도 바꿔 넣게 고침**(지금은 zip 최상위 파일만), `$need` 목록·`windows.yml` zip 목록·SPEC §8.1. CI 에 `claude plugin validate --strict`·`claude plugin test`(한글·공백 경로 사본 포함). 확인: CI.
7. **정리**: `state.rs` carry_args 중복 고침, 화면 문자열·문서의 긴 줄표 정리, 제3자 고지, 버전 3.0.0. 확인: `cargo test`.
8. **문서·배포 준비**: `windows/CHANGELOG.md`·`windows/README.md`·`windows/docs/설치 가이드.md`(Ctrl+Alt+T 가 JetBrains 의 같은 키를 빼앗고 AltGr 배열에서는 그 글자를 못 침), 홈페이지 `site/` 에 Windows 안내를 넣을지(사용자 결정), `gh workflow run windows.yml --ref <브랜치> -f dry_run=true` 로 릴리스 확인. 실제 릴리스는 main 에서만.

## 3. 사용자에게 정해 달라고 할 것

- 작업 브랜치: `feature/embedded-terminal` 에서 이어 갈지, `feature/windows-terminal` 을 새로 딸지.
- sideload ConPTY(`Microsoft.Windows.Console.ConPTY`)를 앱에 실을지(계획 12장의 열린 질문, 권장은 싣기: 시스템 conhost 보다 최신 동작).
- 실제 Windows PC 시험을 사용자가 해 줄 수 있는지, 언제(아래 목록을 그때 넘긴다).
- Windows 3.0.0 을 macOS 3.0.0 과 함께 낼지, 따로 낼지.
- 홈페이지에 Windows 판을 함께 소개할지.

## 4. 실제 Windows PC 로만 확인할 것 (사용자에게 부탁)

- 한글 IME: 조합 중 Enter·Esc·Shift+Enter 가 한 번에 듣는지, 한국어 모드에서 Esc·Ctrl+글자(xterm.js #6065).
- ConPTY 와 Claude Code: 단독 Esc, Shift+Enter 바이트(`\n` 권장, 실기 비교), `/exit` 가 "Background work is running" 대화상자에 막히는지.
- 보드를 끄면 탭 claude 가 5초 안에 끝나고 supervisor·백그라운드 세션은 살아남는지.
- 셸(pwsh/powershell/cmd) 안에서 띄운 claude 가 `/exit` 뒤 프롬프트로 돌아오고 탭 소속이 맞는지.

## 5. 다음 세션 작업 방식

- 메인은 계획·조율·커밋만, 구현은 서브에이전트에 맡기고 결론만 받는다. `windows/` 는 앱(`windows/app/`)과 Rust 크레이트(`windows/crates/`)·패키징이 폴더로 나뉘므로 한 폴더에 한 에이전트.
- 사용자 주간 한도가 넉넉하지 않을 수 있다. 에이전트에게 파일을 통째로 읽지 말고 필요한 부분만 찾아 읽게 하고, CI 를 돌릴 때는 꼭 필요한 때만.
- 단계마다 커밋하고 push 한다(브랜치 push 는 `windows/**` 가 바뀌면 windows.yml CI 가 돈다).
