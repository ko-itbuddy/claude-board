# md-render — 답변 마크다운 꾸미기

보드 탭에서 띄우는 claude 세션에 `--plugin-dir` 로 붙이는 Claude Code mod 다.
답변이 끝나면 제목·굵게·기울임·취소선·인라인 코드·목록 기호·인용·링크를 색으로 다시 그린다(코드 블록·표는 Claude Code 자체 렌더러).

## 출처와 라이선스

- 원본: [deepskyblue86/claude-markdown-mod](https://github.com/deepskyblue86/claude-markdown-mod) (플러그인 이름 `md-render`)
- 들여온 커밋: `c1a064f4a82ade2fdca8e414ac55e89d278c0713` (2026-10-04)
- Copyright (c) 2026 Angelo Puglisi — MIT 라이선스. 원문은 이 폴더의 [LICENSE](LICENSE)(원본 그대로)이고, 이 폴더를 옮기거나 앱에 실을 때 함께 둔다.
- 들여온 파일(원본과 같음): `LICENSE`, `.claude-plugin/plugin.json`, `hooks/hooks.json`, `hooks/register.tsx`, `tests/md-render.test.ts`
- 들여오지 않은 것: `.claude-plugin/marketplace.json`, README·CONTRIBUTING, `tsconfig.json`, `.gitignore`, 그림·예시, CI 설정
- 바꾼 점: 없음 (바꾸면 여기에 적는다)

## 앱에 싣는 파일

`LICENSE`, `README.md`, `.claude-plugin/plugin.json`, `hooks/hooks.json`, `hooks/register.tsx` 를 앱 `Contents/Resources/plugins/md-render/` 에 싣는다(build-board.sh·make-dmg.sh 가 이름으로 골라 복사). `tests/` 는 싣지 않는다.
보드는 시작할 때 이 파일들을 `~/.claude/notifier/plugins/md-render/` 로 옮기고(내용이 같으면 건드리지 않음) 그 경로를 `--plugin-dir` 로 넘긴다 — 아래처럼 Claude Code 가 플러그인 폴더에 파일을 쓰므로 앱 안 경로를 넘기면 서명된 앱이 바뀐다.

## 확인

```sh
claude plugin validate plugins/md-render --strict
claude plugin test plugins/md-render
```

Claude Code 는 대화형 세션에서 사용자 소유 폴더의 플러그인을 불러올 때 이 폴더에 `.claude-plugin/types/`(형 정의, 스스로 `*` 만 든 `.gitignore` 를 가짐)와 맨 위 `tsconfig.json` 을 만든다(`claude -p` 는 만들지 않음, 내용이 같으면 다시 쓰지 않음 — 2.1.302 실측). `types/` 는 git 에 올라가지 않지만 `tsconfig.json` 은 `git status` 에 보이니 올리지 않는다. 앱에는 둘 다 싣지 않는다.
