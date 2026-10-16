#!/bin/bash
# 배포 파일 서명용 개인 인증서(자체 서명)를 만들어 로그인 키체인에 넣고 서명용으로 신뢰한다 — 한 번만 실행한다
# 사용법: make-cert.sh [인증서 이름]   (기본 "Claude Board Signing", make-dmg.sh 는 CLAUDE_BOARD_SIGN_ID 로 다른 이름을 받는다)
# 신뢰 등록 단계에서 macOS 가 로그인 비밀번호 창을 띄운다.
set -e
CN="${1:-Claude Board Signing}"
KC="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning | grep -qF "\"$CN\""; then
  echo "이미 서명용 인증서가 있습니다: $CN"
  exit 0
fi

W="$(mktemp -d)"
trap 'rm -rf "$W"' EXIT
# 앱·dmg 서명용 — 설치 패키지(.pkg) 서명은 Apple 발급 인증서만 허용되므로 넣지 않는다
cat > "$W/cert.cnf" <<EOF
[req]
distinguished_name=dn
x509_extensions=ext
prompt=no
[dn]
CN=$CN
[ext]
basicConstraints=critical,CA:false
keyUsage=critical,digitalSignature
extendedKeyUsage=critical,codeSigning
subjectKeyIdentifier=hash
EOF
# macOS 기본 openssl(LibreSSL) 을 쓴다 — 키체인이 읽는 p12 형식으로 내보낸다
/usr/bin/openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
  -keyout "$W/key.pem" -out "$W/cert.pem" -config "$W/cert.cnf" 2>/dev/null
PW="$(/usr/bin/openssl rand -hex 16)"
/usr/bin/openssl pkcs12 -export -inkey "$W/key.pem" -in "$W/cert.pem" -out "$W/id.p12" -passout "pass:$PW"

echo "1) 로그인 키체인에 인증서·개인 키 등록: $CN"
security import "$W/id.p12" -k "$KC" -P "$PW" -T /usr/bin/codesign -T /usr/bin/productsign >/dev/null

echo "2) 서명용으로 신뢰 등록 — 비밀번호 창이 뜨면 로그인 비밀번호를 입력하세요"
security add-trusted-cert -r trustRoot -p codeSign -p basic -k "$KC" "$W/cert.pem"

# 받는 사람에게 서명자를 확인시켜 줄 때 쓰는 공개 인증서(개인 키 없음)
OUT="$HOME/.claude/notifier/dist"
mkdir -p "$OUT"
cp "$W/cert.pem" "$OUT/$CN.cer"

echo
security find-identity -v -p codesigning | grep -F "\"$CN\""
echo "완료. 공개 인증서: $OUT/$CN.cer"
echo "처음 서명할 때 'codesign(productsign) 이 키체인 키를 사용하려고 합니다' 창이 뜨면 '항상 허용' 을 누르세요."
