#!/bin/sh
# Claude Board.app 을 빌드해 /Applications 에 설치한다 (배포판 pkg 와 같은 위치)
set -e
N="$HOME/.claude/notifier"
APP="/Applications/Claude Board.app"
SIGN_ID="${CLAUDE_BOARD_SIGN_ID:-Claude Board Signing}"
# pkg 로 설치한 앱은 root 소유라 그대로는 덮어쓸 수 없다 — 처음 한 번 관리자 비밀번호 창을 띄워 소유자를 나로 바꾼다
if [ -e "$APP" ] && [ ! -w "$APP/Contents" ]; then
  osascript -e "do shell script \"chown -R $(id -un):staff '$APP'\" with administrator privileges"
fi
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
swiftc -O "$N/ClaudeBoard.swift" -o "$APP/Contents/MacOS/ClaudeBoard"
cp "$N/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Claude Board</string>
  <key>CFBundleDisplayName</key><string>Claude Board</string>
  <key>CFBundleIdentifier</key><string>com.claude.board</string>
  <key>CFBundleExecutable</key><string>ClaudeBoard</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSAppleEventsUsageDescription</key><string>더블 클릭한 세션의 터미널(Ghostty·iTerm2·Terminal)로 포커스를 옮깁니다.</string>
</dict>
</plist>
PLIST
# make-cert.sh 로 만든 개인 인증서가 있으면 그것으로, 없으면 ad-hoc 서명
if security find-identity -v -p codesigning | grep -qF "\"$SIGN_ID\""; then
  codesign --force --timestamp=none --sign "$SIGN_ID" "$APP"
else
  codesign --force --sign - "$APP" >/dev/null 2>&1 || true
fi
touch "$APP"
echo "built: $APP"
