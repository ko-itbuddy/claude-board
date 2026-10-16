#!/bin/sh
# Windows 판 새 버전 준비 — windows/CHANGELOG.md 에 "## <버전>" 항목이 있는지 확인하고 windows/VERSION 을 그 버전으로 바꾼다
# 빌드·릴리스는 이 스크립트가 하지 않는다. 커밋을 main 에 push 하면 .github/workflows/windows.yml 이
# 태그 windows-v<버전> 릴리스를 만든다(절대 Latest 로 표시하지 않는다 — windows/docs/SPEC.md 0.1)
# 사용법: windows/packaging/release.sh <버전>
set -e
VERSION="${1:?사용법: release.sh <버전>}"
W="$(cd "$(dirname "$0")/.." && pwd)"

if ! printf '%s' "$VERSION" | grep -Eq '^[0-9]+(\.[0-9]+){0,2}$'; then
  echo "버전은 2.13 처럼 숫자로 쓰세요: $VERSION" >&2
  exit 1
fi
# CI 와 같은 규칙(packaging/release.sh 와 같은 awk) — 항목이 없으면 CI 의 릴리스 작업이 실패한다
NOTES="$(awk -v v="$VERSION" '/^## /{p = ($2 == v); next} p' "$W/CHANGELOG.md")"
if [ -z "$(printf '%s' "$NOTES" | tr -d '[:space:]')" ]; then
  echo "windows/CHANGELOG.md 맨 위에 '## $VERSION — <날짜>' 항목을 먼저 쓰세요." >&2
  exit 1
fi
# 태그가 이미 있으면 CI 가 릴리스를 건너뛰므로 미리 알린다
if git -C "$W" ls-remote --exit-code --tags origin "refs/tags/windows-v$VERSION" >/dev/null 2>&1; then
  echo "windows-v$VERSION 릴리스가 이미 있습니다. 버전을 올리세요." >&2
  exit 1
fi

OLD="$(tr -d '[:space:]' < "$W/VERSION")"
printf '%s\n' "$VERSION" > "$W/VERSION"
echo "windows/VERSION → $VERSION"
# 문서의 setup.exe·릴리스 링크(windows-v<옛 버전>, ClaudeBoard-<옛 버전>-setup.exe)를 새 버전으로 바꾼다
if [ -n "$OLD" ] && [ "$OLD" != "$VERSION" ]; then
  old_re="$(printf '%s' "$OLD" | sed 's/\./\\./g')"
  for f in "$W/../README.md" "$W/README.md" "$W/docs/설치 가이드.md"; do
    sed -e "s/windows-v$old_re\([^0-9.]\)/windows-v$VERSION\1/g" \
        -e "s/ClaudeBoard-$old_re-setup\.exe/ClaudeBoard-$VERSION-setup.exe/g" "$f" > "$f.tmp"
    mv "$f.tmp" "$f"
  done
  echo "문서의 setup.exe·릴리스 링크 → windows-v$VERSION"
fi
echo "다음: 변경을 커밋(예: \"Windows $VERSION 릴리스 — <요약>\")하고, 브랜치에서 미리 확인하려면"
echo "      gh workflow run windows.yml --ref <브랜치> -f dry_run=true   (아무것도 만들지 않음)"
echo "      main 에 push 하면 CI 가 빌드한 뒤 windows-v$VERSION 릴리스를 Latest 표시 없이 만듭니다."
