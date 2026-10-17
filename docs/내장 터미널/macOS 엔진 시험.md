# 보드 내장 터미널 — macOS 엔진 시험 결과 (libghostty-spm)

> 이 보고서가 가리키는 `$S`(당시 임시 폴더)의 측정 기록(`out/*.log`·`*.jsonl`·`*.png`), 래퍼 소스 사본(`ref/libghostty-spm`), 시험 빌드는 남아 있지 않다. 수치는 본문 표에 있고, 시험 앱 소스와 도구는 [시험 도구/](시험%20도구/) 에 옮겨 두었다. 래퍼 소스는 `libghostty-spm` 태그 `2.2.2026101502` 를 다시 받으면 된다.

2026-10-17 · 이 Mac: macOS 26.5.2, arm64, Command Line Tools 만(Swift 6.3.2), Xcode 없음.
시험 앱은 버리는 코드다. 저장소 파일은 하나도 고치지 않았고 `claude` 는 실행하지 않았다(대신 zsh 와 바이트 기록기 `scripts/rec.py`).
아래에서 `$S` 는 `/private/tmp/claude-501/-Users-max-mackbook-pro-local-workspace-claude-board/46cf03bc-65b6-46f6-86c6-d810b037f6f0/scratchpad/spike-macos` 이다.
(서브에이전트가 보고서 파일 쓰기를 막혀 메인이 본문을 받아 저장함)

## 고정한 버전
- `libghostty-spm` **2.2.2026101502** (`exact:`) — 1.5.2 이상 태그 중 최신(2026-10-16 발행). 바이너리는 Ghostty 커밋 `1e9e14af5df1`(자산 `upstream.1e9e14af5df1-3`, checksum 고정).
  - 2.2.2026101501 까지는 libghostty.a 에 LGPL 인 GNU libintl 이 정적으로 들어 있었다(패키지 문서의 issue #88). 이번 판 macOS 조각과 시험 빌드 결과물에서 libintl 기호 0개를 `nm` 으로 확인 → **이 판 이상으로 고정해야 한다.**
- 따라 오는 의존성 DisplayLink 3.0.1 (`Package.resolved` 에 고정).

## 항목별 결과

| # | 항목 | 결과 | 근거(수치·파일) |
|---|---|---|---|
| 1 | CLT 만으로 빌드·universal·크기 | **됨**(조건 2개) | `swift build -c release` 성공(첫 빌드 56~78초, x86_64 72~90초, 보드 파일만 고친 재빌드 18~22초). 단 **`--disable-keychain` 필수** — 빼면 바이너리 자산 내려받기에서 키체인 자격 증명 조회가 막혀 10분 넘게 진행 0(`resolve-v.log`). `--arch arm64 --arch x86_64` 는 **안 됨**(xcbuild=Xcode 필요). 아키텍처별 `--triple` 빌드 + `lipo` 는 **됨**, x86_64 조각도 Rosetta 에서 터미널까지 정상(`out/multi-2.log`). 크기는 아래 표 |
| 2 | 단일 파일 빌드 방식·리소스 위치 | **둘 다 됨, SwiftPM 실행 타깃 권장** + 리소스 번들은 **우회 필요** | swiftc + SwiftPM 산출물 연결: 됨(15.1 MB) 하지만 SwiftPM 내부 경로(`.build/<triple>/release/*.build/*.o`, `Modules/`)에 기댐. SwiftPM 실행 타깃: 실제 `ClaudeBoard.swift`(최상위 코드, 이름이 main.swift 아님)를 고치지 않고 유일한 소스로 빌드됨(13.0 MB). 저장소 맨 위 배치(`path: "."`, `sources: ["ClaudeBoard.swift"]`)도 됨. 리소스 번들 문제와 해결은 "막힌 점 1" |
| 3 | 터미널 여러 개·셸+첫 입력·경로·환경 변수 | **됨** | 컨트롤러 1개로 zsh 5개 + 명령 1개 + 크기 0 뷰 1개. 붙는 데 0.12~0.15초. 작업 경로·`CLAUDE_BOARD_TERMINAL` 5/5 일치(`out/multi-5.log`, `out/multi-*.txt`). 첫 입력 `paste(text:)` + `sendKey(.enter)` 5/5 성공 — 붙자마자(셸 준비 전 0.19초) 넣어도, 2초 뒤 넣어도 실행. 셸 준비 전에 넣으면 같은 명령이 두 번 찍혀 보이는 겉모양 문제만(`out/multi-5-t0.png`) |
| 4 | 숨긴 터미널의 출력 처리 | **됨(조건)** — 뷰를 창에서 **전부 빼면 막힘** | 초당 40 KB + 제목 변경(OSC 2) 발생기로 20초 숨김: `isHidden`·`setSurfaceVisible(false)`·둘 다·창 `orderOut` 모두 write 지연 최대 0.001초, 다시 보이면 마지막 출력까지 화면에 있음(`out/hidden-both-shown.png`). **모든 터미널 뷰를 `removeFromSuperview` 하면 자식의 write 가 19.1초 멈춤**(`out/hidden-remove-stream.jsonl`). 제목 변경 없이 글자만 내면 안 멈춤(`out/hidden-remove-notitle.log`) |
| 5 | 자원(RSS·놀 때 CPU) | **측정함** | 아래 표. 첫 터미널이 약 +150 MB(GPU·IOSurface 포함 footprint), 그 뒤 숨긴 터미널마다 +3~4 MB, 놀 때 CPU 0.35~0.45%. `isHidden` 만 하면 숨긴 터미널도 계속 그려 footprint 2배·CPU 3배 |
| 6 | 자식 pid·tty 짝짓기 | **됨** | `foregroundPid`·`ttyName` 이 실제 값을 줌(래퍼 주석 "1.3.1 에서 항상 nil" 은 이 판에선 틀림). `ttyName` `/dev/ttys005` ↔ state.py 의 `tty` `ttys005`(`/dev/` 만 떼면 됨). `foregroundPid` 는 `tcgetpgrp` — 프롬프트에선 zsh pid, 앞에서 도는 프로그램이 있으면 그 pid(sh→exec sleep 로 5/5 일치). 환경 변수 방식도 5/5 전달 |
| 7 | 키 바이트 | **됨** | 공개 API(`sendKey`)와 실제 키 경로(`keyDown`)가 모든 모드에서 같은 바이트. 기본 설정의 Shift+Enter 는 `1b 5b 32 37 3b 32 3b 31 33 7e`(CSI 27;2;13~), 사용자 설정(`keybind = shift+enter=text:\x1b\r`)을 넣으면 `1b 0d` — **Ghostty.app(사용자 설정)에 AppleScript 로 같은 키를 보낸 실측과 일치**. 표는 아래 |
| 8 | 한글 조합 입력 | **부분** | 입력기 호출 흉내: 조합 중 밑줄 표시(`out/ime-marked-han-crop.png`), 조합 중 바이트 없음, 확정 시 `ed 95 9c`(한)+`20`. 조합 중 `sendKey(.enter)` 는 조합을 먼저 확정하고 Enter(`e3 84 b1`, `0d`). **위험**: Return 한 번에 입력기가 "글자 확정 + 줄바꿈 명령"을 함께 부르면 글자만 가고 Return 이 사라짐(`ea b0 81` 만, `out/ime.log`). keyDown 밖에서 확정된 글자는 붙여넣기(bracketed paste)로 감. 실제 입력기는 합성 키로 움직일 수 없었음(앱 입력 문맥에 2벌식을 골라도 글자가 그대로 통과, `out/ime-real.log`) → **사람이 직접 쳐 봐야 함** |
| 9 | 사용자 Ghostty 설정 읽기 | **부분** | 설정 위치: `~/.config/ghostty/` 는 비어 있고 `~/Library/Application Support/com.mitchellh.ghostty/config.ghostty` 에 있음. `TerminalController(configFilePath:)` 로 읽으면 **`theme = "Catppuccin Mocha"` 한 줄 때문에 파일 전체가 거부**되고 기본값으로 조용히 돌아감(번들에 themes 폴더 없음, `lastConfigurationIssue` 에만 남음). 나머지 11줄은 줄마다 따로 넣어도 진단 0 — 트림 빌드라 거부되는 키는 없음. theme 줄을 빼고 `GhosttyThemeCatalog` 의 같은 이름 테마를 얹으면 배경 `#1e1e2e`·글꼴 JetBrainsMono NFM 15pt 적용(셀 15×35 px, 기본 17×37) — `out/config.log`, `out/config-fonts.png` |
| 10 | 클립보드 | **됨(조건)** | 처리기 없이도 Cmd+C(선택 복사)·Cmd+V(한 줄, bracketed paste 가 켜진 프로그램에선 여러 줄도) 동작. **bracketed paste 가 꺼진 프로그램에 여러 줄을 붙이면 알림 없이 버려짐**(붙여넣기 보호가 확인을 요청 → 처리기 없음 → 거부). 프로그램의 OSC 52 쓰기는 묻지 않고 클립보드를 덮어씀, OSC 52 읽기는 거부(빈 응답). 처리기를 달면 두 경우 모두 물어 옴(`out/clip-none.log`, `out/clip-allow.log`, `out/clip-none-bp.log`) |
| 11 | 프로그램 종료·제목 콜백 | **됨** | 제목(OSC 2) → `TerminalSurfaceTitleDelegate`. 셸 통합으로 작업 경로(OSC 7)·명령 끝(OSC 133, 종료 코드·걸린 시간) 콜백도 옴. 셸에서 `exit` → `terminalDidClose(processAlive: false)` 즉시(`out/exit.log`), 화면 뷰는 호스트가 치울 때까지 남음. `command:` 로 띄운 터미널은 `waitAfterCommand: false` 여도 끝나면 "Process exited. Press any key…" 를 띄우고 키를 누를 때까지 닫힘 콜백 없음. 바로(54 ms 에) 0 아닌 코드로 끝난 명령은 "Ghostty failed to launch the requested command" 화면(기준은 설정 기본값 250 ms 로 추정) |

### 크기 (항목 1)

| 실행 파일 | arm64 | x86_64 | universal |
|---|---|---|---|
| 지금 보드 (`/Applications/Claude Board.app`) | — | — | 7,495,136 B (7.5 MB) |
| 보드 소스 + 엔진, SwiftPM 실행 타깃(`$S/board-root`) | 13,046,648 B | 13,594,264 B | **26,661,752 B (26.7 MB)** |
| 보드 소스 + 엔진, swiftc 연결(`$S/board-ga`) | 15,122,840 B | — | — |
| 시험 앱 | 10.4 MB (strip 9.4 MB) | 11.0 MB | 21.5 MB (strip 19.4 MB) |

- 리소스 번들 `GhosttyKit_GhosttyTerminal.bundle` 124 KB(셸 통합 5개 파일, terminfo, 지역화 문자열). 시험 `.app` 전체 10 MB.
- 첫 빌드 때 내려받는 xcframework zip 76.5 MB(모든 플랫폼), 풀면 `.build/artifacts` 187 MB. macOS 조각 `libghostty.a` 40.7 MB(arm64+x86_64). 링크 결과는 시스템 dylib 만 씀(엔진은 정적).

### 자원 (항목 5) — 시험 앱 프로세스, 20초 동안 놀 때

| 터미널 수 | 숨긴 방식 | footprint | RSS | 놀 때 CPU |
|---|---|---|---|---|
| 0 (창·컨트롤러만) | — | 14.9 MB | 80 MB | 0.02% |
| 1 | — | 167.0 MB | 91 MB | 0.44% |
| 5 | 1개 보임, 4개 `isHidden`+`setSurfaceVisible(false)` | 173.1 MB | 98 MB | 0.35% |
| 10 | 1개 보임, 9개 같은 방식 | 188.2 MB | 88 MB | 0.45% |
| 5 | 4개 `isHidden` 만 | 338.1 MB | 101 MB | 1.21% |

- footprint 는 `task_info(TASK_VM_INFO).phys_footprint`(활동 모니터의 "메모리", GPU·IOSurface 포함). 비교: 지금 떠 있는 보드의 footprint 166 MB.
- 터미널마다 셸 쪽 프로세스(`/usr/bin/login` → `-zsh` + 사용자 p10k 의 gitstatusd 등)는 별도 프로세스(5개일 때 직계 자식 login 들의 RSS 합 44~54 MB). Ghostty.app 에서 같은 셸을 띄울 때와 같은 몫.
- 스크롤백은 출력이 쌓이는 만큼 터미널마다 늘어남(Ghostty 기본 상한 10 MB/터미널 — 설정 기본값 기준 추론, 측정 안 함).

### 숨김 방식별 (항목 4) — 출력 발생기 1개 + zsh 1개, 둘 다 20초 숨김

| 숨긴 방식 | write 가 막힌 최대 시간 | 숨긴 동안 CPU(10초) | 다시 보일 때 |
|---|---|---|---|
| 숨기지 않음 | 0.000초 | 0.917초 | — |
| `isHidden = true` 만 | 0.001초 | 1.055초 (계속 그림) | 마지막 출력까지 있음 |
| `setSurfaceVisible(false)` 만 | 0.000초 | 0.063초 | 있음 |
| 둘 다 | 0.000초 | 0.061초 | 있음 |
| 창 `orderOut` | 0.001초 | 0.966초 (계속 그림) | 있음 |
| **모든 뷰 `removeFromSuperview`** | **19.131초** | 0.002초 | 다시 붙인 뒤에야 이어짐 |
| 모든 뷰 빼기, 제목 변경 없는 출력 | 0.000초 | 0.511초 | 있음 |

원인(소스 + 실측): 래퍼는 창에 붙은(`window != nil`) 터미널이 **하나도 없으면** 엔진의 앱 메일함을 비우는 tick 을 멈춘다(`TerminalController.handleWakeup`, 관찰자의 `shouldProcess = isApplicationActive && isAttached()`). 제목 변경 같은 알림이 메일함에 쌓여 가득 차면 엔진의 pty 읽기 스레드가 막히고, pty 버퍼가 차서 터미널 안 프로그램의 write 가 멈춘다. 숨긴 뒤 제목 변경 약 35번 만에 멈췄다. Claude Code 는 작업 중 제목을 자주 바꾸므로 그대로 해당된다.

### 키 바이트 (항목 7) — 터미널 안 프로그램이 받은 hex

| 키 | 기본 설정, 레거시 | 사용자 설정, 레거시 | Ghostty.app(사용자 설정), 레거시 | kitty 키보드 프로토콜(flags 1) |
|---|---|---|---|---|
| Esc | `1b` | `1b` | `1b` | `1b 5b 32 37 75` |
| Enter | `0d` | `0d` | `0d` | `0d` |
| Shift+Enter | `1b 5b 32 37 3b 32 3b 31 33 7e` | `1b 0d` | `1b 0d` | 기본 `1b 5b 31 33 3b 32 75` / 사용자 설정·Ghostty.app `1b 0d` |
| Ctrl+C | `03` | `03` | `03` | `1b 5b 39 39 3b 35 75` (Ghostty.app 은 AppleScript 경로라 아무것도 안 옴 — AppleScript 흉내의 한계로 봄) |
| paste "abc" | `61 62 63` (bracketed paste 켜면 `1b 5b 32 30 30 7e … 1b 5b 32 30 31 7e`) | 같음 | 같음 | — |
| paste "x\ny" | `78 0d 79` (켜면 `… 78 0a 79 …`) | 같음 | 같음 | — |

- 시험 앱 열은 공개 API(`sendKey`·`paste`)와 실제 키 경로(합성 `NSEvent` → `keyDown`)가 모든 칸에서 같았다. 전체: `out/keys-default-*.jsonl`, `out/keys-user-*.jsonl`, `out/ghostty-app-*.jsonl`(정리 `scripts/analyze_keys.py`). kitty flags 31 결과도 파일에 있음.
- Ghostty.app 열은 사용자의 실제 Ghostty(1.3.1)에 AppleScript `send key` 로 보낸 실측(물리 키보드 아님). 기본 설정의 Ghostty.app 값은 재지 않음.

## 막힌 점과 우회법

1. **리소스 번들을 .app 에 넣는 길이 없음(CLT 의 SwiftPM).** CLT 의 SwiftPM 이 만드는 `Bundle.module` 은 `Bundle.main.bundleURL/GhosttyKit_GhosttyTerminal.bundle`(= `.app` 맨 위) → 없으면 **빌드한 컴퓨터의 절대 경로**만 찾고, 둘 다 없으면 `fatalError`. `.app` 맨 위에 두면 `codesign` 이 "unsealed contents present in the bundle root" 로 거부(심볼릭 링크도 같음).
   - 빌드 경로가 남아 있는 개발 Mac 에서는 멀쩡히 돌고 **사용자 Mac 에서만 첫 터미널을 만들 때 죽는다.** 빌드 경로 번들을 치우고 실행해 그대로 재현(`could not load resource bundle`).
   - 우회(실측 통과): 번들은 `Contents/Resources/GhosttyKit_GhosttyTerminal.bundle` 에 두고, 앱이 처음 `TerminalController` 를 만들기 전에 **main 번들의 `bundleURL` 을 잠깐 `Contents/Resources` 로 돌려준 상태에서 `GhosttyRuntimeResources.directoryURL` 을 한 번 읽고 되돌린다**(Objective-C 메서드 교체 약 15줄, 시험 앱의 `resolveResourceBundleInApp()`). `Bundle.module` 은 한 번 정해지면 바뀌지 않아 그 뒤로 `.app` 안을 쓴다. 결과: `codesign --verify --strict` 통과, 셸 안 `TERMINFO`·`GHOSTTY_RESOURCES_DIR` 가 `.app` 안을 가리킴, `infocmp xterm-ghostty` 성공, 셸 통합(제목·명령 끝 콜백) 동작 — 빌드 경로 번들을 치운 상태에서 확인(`out/multi-1.log`).
   - 다른 길: 정확한 태그의 로컬 사본(`path:`)에서 `.module` 쓰는 7곳을 `Bundle.main.resourceURL` 먼저 보게 고치기, 또는 upstream 에 "리소스 위치를 앱이 지정" 요청. 메서드 교체가 가장 작고 원격 `exact:` 고정을 그대로 둔다.
   - `.app` 안 배치: `Contents/Resources/GhosttyKit_GhosttyTerminal.bundle/{Info.plist, Ghostty/shell-integration/{bash,zsh}/…, terminfo/{67/ghostty, 78/xterm-ghostty}, *.lproj/Localizable.strings}`. 아키텍처와 무관해 universal 이어도 한 벌.
2. **SwiftPM 이 키체인에서 멈춤** → 모든 `swift package`/`swift build` 에 `--disable-keychain`.
3. **universal 은 한 번에 안 됨** → `swift build -c release --triple arm64-apple-macosx14.0` 과 `--triple x86_64-apple-macosx14.0` 를 따로 하고 `lipo -create`. 한 `.build` 에서 두 트리플을 오갈 땐 매번 `--triple` 을 적어야 함(호스트 기본값과 섞으면 "not registered" 오류).
4. **창에서 모든 터미널 뷰를 빼면 세션이 멈춤**(항목 4) → "터미널 펼치기" 를 끌 때 뷰를 빼지 말고 창 안에 둔 채 `isHidden = true` + `setSurfaceVisible(false)`. 창을 숨기거나 최소화할 때도 `setSurfaceVisible(false)` 를 직접 불러야 그리기가 멈춤(`orderOut` 만으로는 계속 그림).
5. **크기 0 인 뷰는 셸을 띄우지 않음**(크기를 준 순간 시작, `out/multi-5.log` 의 zero) → 보드가 다시 떠서 접힌 상태로 세션을 이어 열 때도 터미널 뷰에 실제 크기를 줘야 함. 숨긴 뷰를 작게 줄이면 pty 크기가 바뀌어 프로그램이 다시 그리므로 마지막 크기 유지가 낫다. 뷰가 해제되면 셸도 끝나므로 보드가 뷰를 계속 쥐고 있어야 함.
6. **`initial_input` 을 래퍼가 내보내지 않음**(C 구조체엔 있음) → `paste(text:)` + `sendKey(.enter)` 로 충분. 두 번 찍혀 보이는 게 싫으면 셸 통합의 첫 작업 경로 콜백(붙은 뒤 0.35~1.3초)을 기다렸다 넣는다. 또는 `command: "/bin/zsh -lc '<명령>; exec /bin/zsh -l'"`(됨, 단 명령이 셸 기록에 안 남음).
7. **사용자 Ghostty 설정을 그대로 못 씀**(항목 9) → 보드가 설정을 직접 만든다: 사용자 파일에서 `theme` 줄을 빼고 `GhosttyThemeCatalog.theme(named:)` 로 옮기거나, 글꼴·크기·키 설정만 골라 `TerminalConfiguration` 으로 넣는다. `TerminalController(configFilePath:)` 의 `theme` 인자를 생략하면 래퍼 기본 테마(Afterglow/Alabaster)가 사용자 색을 덮으니 `TerminalTheme()`(빈 값)나 카탈로그 테마를 꼭 넘긴다.
8. **Shift+Enter 바이트가 설정에 따라 다름**(항목 7) → 보드 설정에 `keybind = shift+enter=text:\x1b\r` 를 명시(사용자 Ghostty 와 같게).
9. **붙여넣기 보호 확인을 처리하지 않으면 조용히 버려짐**(항목 10) → Ghostty.app 처럼 `TerminalSurfaceClipboardConfirmationDelegate` 로 확인 창.

## 보드에 넣을 때 권장 빌드 방식

**SwiftPM 실행 타깃 하나**(위 2번의 권장안). swiftc 에 SwiftPM 산출물을 붙이는 방식은 SwiftPM 내부 산출물 경로와 개체 파일 목록에 기대 업데이트 때 깨지기 쉽다.

- 저장소 맨 위에 `Package.swift`, 파일은 옮기지 않음(실측): `executableTarget(name: "ClaudeBoard", dependencies: [.product(name: "GhosttyTerminal", package: "libghostty-spm")], path: ".", exclude: [windows·packaging·다른 .swift·.py 등], sources: ["ClaudeBoard.swift"], swiftSettings: [.swiftLanguageMode(.v5)])` + `.package(url: "https://github.com/Lakr233/libghostty-spm.git", exact: "2.2.2026101502")`. `exclude` 가 없으면 "unhandled files" 경고만 난다. `ClaudeBoard.swift` 는 최상위 코드 그대로 빌드된다(파일을 여러 개로 나누면 최상위 코드는 `main.swift` 에만).
- `.gitignore` 에 `!/Package.swift`, `!/Package.resolved` 추가(`.build/` 는 지금 규칙대로 빠짐). `Package.resolved` 를 커밋해 DisplayLink 도 고정.
- `build-board.sh`: `swiftc -O ClaudeBoard.swift …` → `swift build -c release --disable-keychain --triple arm64-apple-macosx14.0`, `.build/arm64-apple-macosx/release/ClaudeBoard` 는 `Contents/MacOS/`, `GhosttyKit_GhosttyTerminal.bundle` 은 `Contents/Resources/` 로 복사 → 서명(지금처럼 `--deep` 없이 통과).
- `packaging/make-dmg.sh`: 두 트리플 빌드 + `lipo`(지금의 `swiftc -target …` 두 번 + `lipo` 와 같은 모양), 번들은 한 벌.
- 앱 코드: 첫 `TerminalController` 전에 리소스 번들 우회(막힌 점 1) 한 번.
- 바뀌는 작업 흐름: 첫 빌드에 네트워크(76.5 MB), 깨끗한 빌드 약 1분 20초(x86_64 약 1분 30초), 보드 파일만 고친 재빌드 약 18~22초. **CLAUDE.md 의 `swiftc -typecheck ClaudeBoard.swift` 는 `import GhosttyTerminal` 뒤로는 못 쓰고 `swift build` 로 바뀐다.** `ClaudeNotifier.swift` 등 다른 실행 파일은 지금처럼 swiftc.

## 설계 메모에 반영할 것

- 설계를 뒤집을 문제는 없다. 엔진이 pid·tty 를 주므로 **세션 ↔ 터미널 짝짓기는 1순위(파일 약속 변경 없음)로 된다** — `ttyName` 에서 `/dev/` 를 뗀 값 = 세션 파일 `tty`. 앞에서 도는 프로그램이 claude 면 `foregroundPid` = 세션 파일 `pid` 인데, 이 마지막 등식은 claude 가 셸의 작업 대표 프로세스로 뜬다는 추론이다(claude 로는 재지 않음).
- 다만 구현 제약 셋: (1) 접힌 상태에서도 터미널 뷰를 창 안에 실제 크기로 숨겨 둘 것, (2) `.app` 리소스 번들 우회, (3) 빌드가 SwiftPM 으로 바뀜.
- "엔진 정확한 버전 고정" 은 2.2.2026101502 이상이어야 LGPL 정적 연결이 없다.
- 한글 조합 중 Return 이 한 번에 먹는지는 사람이 확인해야 한다(아래). 래퍼의 입력 코드는 Ghostty.app 의 AppKit 코드를 옮긴 것이라 지금 Ghostty.app 에서 겪는 것과 같을 것으로 본다(추론).

## 사용자가 직접 한글을 쳐 볼 실행 명령

    /private/tmp/claude-501/-Users-max-mackbook-pro-local-workspace-claude-board/46cf03bc-65b6-46f6-86c6-d810b037f6f0/scratchpad/spike-macos/run-ime-test.sh

- 창 하나: 왼쪽은 받은 바이트를 `got: <글자>   hex: <바이트>` 로 한 줄씩 보여 주는 기록기(bracketed paste 켬), 오른쪽은 zsh. 창을 닫으면 끝(30분 뒤 저절로 끝).
- 확인할 것:
  1. 왼쪽을 누르고 한글 입력기로 "한글" → 치는 동안 커서 자리에 밑줄 친 조합 글자가 보이고, 확정되면 `ed 95 9c`(한) `ea b8 80`(글).
  2. "한" 을 친 뒤 스페이스 없이 바로 Return → `ed 95 9c` 와 `0d` 가 **둘 다** 오는지. `0d` 가 안 오면 Return 이 사라지는 것 — 같은 동작을 Ghostty.app 에서도 해 보고 비교.
  3. 조합 중 Esc·Backspace, 한/영 전환 직후 입력, 오른쪽 zsh 에서 한글 입력·지우기.
- 기록: `$S/out/ime-manual-rec.jsonl`. 이 경로는 이 대화의 임시 폴더라 나중에 지워질 수 있다.

## 시험 범위 밖에 남긴 것

- SwiftPM 이 처음 실행될 때 전역 캐시를 만들었다(오늘 13:26 생성, 시험 폴더 밖): `~/.swiftpm`, `~/Library/Caches/org.swift.swiftpm`(76 MB), `~/Library/org.swift.swiftpm`. 보드를 SwiftPM 으로 빌드하면 어차피 생기므로 지우지 않았다. 지우려면 `rm -rf ~/.swiftpm ~/Library/Caches/org.swift.swiftpm ~/Library/org.swift.swiftpm`.
- Ghostty.app 비교 때 사용자 Ghostty 에 창이 잠깐(각 약 4초) 두 번 열렸다 닫혔다(남은 시험 창 없음 확인).
- 사용자 클립보드는 시험 전 비어 있었고 시험 뒤 다시 비웠다.
- 남은 시험 프로세스 없음(`lsof`·`pgrep` 확인).

## 파일

- 시험 앱 소스 `$S/pkg/Sources/Spike/main.swift`, 패키지 `$S/pkg/Package.swift`(+ `Package.resolved`)
- 보조 스크립트 `$S/scripts/rec.py`(바이트 기록기), `stream.py`(출력 발생기), `analyze_keys.py`, `ghostty_keys.py`
- 서명한 시험 `.app` `$S/app/SpikeTest.app`, universal 산출물 `$S/dist/`
- 보드 소스로 빌드 방식 시험: `$S/board-na`(SwiftPM 하위 폴더 배치), `$S/board-root`(SwiftPM 저장소 맨 위 배치), `$S/board-ga`(swiftc 연결)
- 측정 기록 `$S/out/*.log`, `*.jsonl`, `*.png`
- 읽은 래퍼 소스(태그 사본) `$S/ref/libghostty-spm`
