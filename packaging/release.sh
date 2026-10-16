#!/bin/sh
# 새 버전을 GitHub 릴리스로 올린다 — make-dmg.sh 로 앱을 빌드·서명하고, 앱 zip·install.sh·VERSION 을 v<버전> 릴리스에 붙인다
# 보드의 업데이트 확인과 install.sh 는 늘 최신 릴리스(releases/latest/download/...)의 이 세 파일을 받는다 — 파일 이름을 바꾸지 않는다
# 릴리스 태그는 저장소 기본 브랜치의 HEAD 에 만들어지므로 소스를 먼저 push 해 둔다. gh auth login 으로 로그인해 두어야 한다
# 릴리스 설명은 CHANGELOG.md 의 "## <버전>" 항목을 그대로 쓴다 — 항목이 없으면 빌드하기 전에 멈춘다
# 사용법: release.sh <버전>
set -e
VERSION="${1:?사용법: release.sh <버전>}"
REPO="suhojang/claude-board"
N="$(cd "$(dirname "$0")/.." && pwd)"
P="$N/packaging"

NOTES="$(awk -v v="$VERSION" '/^## /{p = ($2 == v); next} p' "$N/CHANGELOG.md")"
if [ -z "$(printf '%s' "$NOTES" | tr -d '[:space:]')" ]; then
  echo "CHANGELOG.md 에 '## $VERSION' 항목을 먼저 쓰세요." >&2
  exit 1
fi

"$P/make-dmg.sh" "$VERSION"

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
cp "$N/dist/ClaudeBoard-$VERSION-app.zip" "$W/ClaudeBoard.zip"
cp "$P/install.sh" "$W/install.sh"
printf '%s\n' "$VERSION" > "$W/VERSION"
gh release create "v$VERSION" "$W/ClaudeBoard.zip" "$W/install.sh" "$W/VERSION" \
  --repo "$REPO" --title "Claude Board $VERSION" --notes "$NOTES"
echo "released: https://github.com/$REPO/releases/tag/v$VERSION"
