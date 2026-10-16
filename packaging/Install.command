#!/bin/bash
# Claude Board 설치 — 보드 앱·알림 패널을 설치하고 전역 ~/.claude/settings.json 에 hook 을 등록한다
set -e
DIR="$(cd "$(dirname "$0")" && pwd)"
SRC_APP="$DIR/Claude Board.app"
RES="$SRC_APP/Contents/Resources/notifier"
APP="$HOME/Applications/Claude Board.app"
N="$HOME/.claude/notifier"
BIN="$HOME/.local/bin"

echo "== Claude Board 설치 =="
if ! /usr/bin/env python3 -c 'import sys' >/dev/null 2>&1; then
  echo "python3 를 찾지 못했습니다. hook 이 python3 로 실행되므로 먼저 설치하세요(xcode-select --install 또는 Homebrew)."
  exit 1
fi
[ -d "$SRC_APP" ] || { echo "dmg 안의 'Claude Board.app' 을 찾지 못했습니다."; exit 1; }

echo "1) 앱 설치: $APP"
pkill -x ClaudeBoard 2>/dev/null || true
mkdir -p "$HOME/Applications"
rm -rf "$APP"
cp -R "$SRC_APP" "$APP"
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

echo "2) 알림 패널·hook 스크립트 설치: $N"
mkdir -p "$N/sessions"
for f in claude-notifier notify.py state.py board-tui.py hooks.py; do
  cp "$RES/$f" "$N/$f"
done
chmod +x "$N/claude-notifier" "$N/notify.py" "$N/state.py" "$N/board-tui.py"
xattr -d com.apple.quarantine "$N/claude-notifier" 2>/dev/null || true

echo "3) 전역 settings.json 에 hook 등록"
python3 "$N/hooks.py" install

echo "4) 명령 설치: $BIN/claude-board, $BIN/claude-board-tui"
mkdir -p "$BIN"
printf '#!/bin/sh\n# Claude Board 앱을 연다 — 이미 떠 있으면 창을 앞으로 불러온다\nopen -a "$HOME/Applications/Claude Board.app"\n' > "$BIN/claude-board"
chmod +x "$BIN/claude-board"
ln -sf "$N/board-tui.py" "$BIN/claude-board-tui"

echo "5) 로그인 항목 등록"
osascript -e 'tell application "System Events" to delete (every login item whose name is "Claude Board")' >/dev/null 2>&1 || true
osascript -e "tell application \"System Events\" to make login item at end with properties {path:\"$APP\", hidden:false}" >/dev/null 2>&1 \
  || echo "   (로그인 항목 등록 실패 — 시스템 설정 > 일반 > 로그인 항목에서 직접 추가하세요)"

if [ -d "/Applications/Ghostty.app" ] || [ -d "$HOME/Applications/Ghostty.app" ]; then
  if ! grep -q ">>> claude-board >>>" "$HOME/.zshrc" 2>/dev/null; then
    printf "6) Ghostty 를 열 때 보드가 꺼져 있으면 함께 띄우는 줄을 ~/.zshrc 에 추가할까요? [Y/n] "
    read -r ans || ans=""
    if [ -z "$ans" ] || [[ "$ans" =~ ^[Yy] ]]; then
      cat >> "$HOME/.zshrc" <<'ZSH'

# >>> claude-board >>>
# Ghostty 에서 셸이 시작될 때 보드 앱이 꺼져 있으면 함께 띄운다 (포커스는 터미널 유지)
if [[ "$TERM_PROGRAM" == "ghostty" ]] && ! pgrep -xq ClaudeBoard; then
  open -g -a "$HOME/Applications/Claude Board.app" --args --background
fi
# <<< claude-board <<<
ZSH
      echo "   추가했습니다."
    fi
  fi
fi

open "$APP"
echo
echo "설치를 마쳤습니다. 이미 실행 중인 Claude Code 세션은 다시 시작해야 hook 이 적용됩니다."
echo "처음 알림·더블 클릭 때 macOS 가 'Ghostty 제어' 권한을 물으면 허용하세요."
