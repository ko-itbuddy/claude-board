# state hook 기준 데이터 (14단계)

macOS `state.py`(커밋 `68ffed2`)에 hook 이벤트 14개를 차례로 넣고, 이벤트마다 세션 파일을 남긴 REA 프로세스 캡처다.
근거: REA evidence `ev_23674ebbd76cceea4252d166b60c1ad66648ee3f2a951ba5d4c669d1c07d3368` (SPEC §3.2).

- `NN-<이름>.in.json` — 그 단계의 입력: `state`(hook 인자 `state.py <state>`)와 `input`(stdin 으로 넣은 hook 입력 JSON).
  캡처 스크립트 `drive.sh` 의 `step` 줄을 그대로 풀어 만든 것이다(`$base` = `session_id`·`transcript_path`·`cwd`).
- `NN-<이름>.out.json` — 그 단계 뒤 `sessions/<session_id>.json` 원본(바이트 그대로 복사).

## 가린 필드

캡처는 실행 중인 Claude Code 세션 안에서 돌았으므로, 조상에서 찾은 `claude` 프로세스와 그 인자는 시나리오가 아니라 그 세션의 것이다.
그래서 `masked_sequence_matches_state_py` 시험은 다음 필드를 양쪽에서 **키째 지우고** 비교한다(값을 바꾸지 않는다 — `pid_ctime` 은 macOS 기록에 없다).

- 프로세스: `pid`, `pid_ctime`, `tty`, `headless`, `args`, `resume_cmd`, `resume_args`(Windows 판이 더한 필드 — macOS 기록에 없다)
- 시계: `started`, `updated`, `since`, `subagents.*.started`·`updated`·`ended`, `limit_hit.at`
- 기기 경로: `transcript_path`

`replay_with_captured_clock_and_process_is_exact` 시험은 캡처와 같은 가짜 프로세스(pid 46137, tty `ttys000`, 인자 `--allow-dangerously-skip-permissions`)와
단계마다 캡처의 `updated` 를 시계로 넣어, 가린 필드까지 모두 같고 키 순서·터미널 제목·`.paused` 삭제 여부도 맞는지 본다.
`resume_args` 는 내용(`resume_cmd` 와 같은 인자 배열)을 따로 확인한 뒤 빼고 비교한다.
