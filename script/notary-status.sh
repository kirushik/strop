#!/usr/bin/env bash
# notary-status — query Apple's Notary REST API from any OS (no Mac, no
# xcrun): list recent notarization submissions and their status, or fetch
# one submission's log. notarytool is only a client for this same API;
# the App Store Connect .p8 key signs a short-lived ES256 JWT locally.
#
# usage:
#   APPLE_API_KEY=path/to/AuthKey_XXXX.p8 \
#   APPLE_API_KEY_ID=XXXXXXXXXX \
#   APPLE_API_ISSUER=uuid \
#     script/notary-status.sh              # list recent submissions
#     script/notary-status.sh <submission-id>   # one submission + its log URL
set -euo pipefail

for v in APPLE_API_KEY APPLE_API_KEY_ID APPLE_API_ISSUER; do
  [[ -n ${!v:-} ]] || { echo "$v is required" >&2; exit 2; }
done
[[ -f $APPLE_API_KEY ]] || { echo "APPLE_API_KEY must be a .p8 file path" >&2; exit 2; }

token=$(python3 - "$APPLE_API_KEY" "$APPLE_API_KEY_ID" "$APPLE_API_ISSUER" <<'PY'
import sys, time, jwt
key_path, kid, issuer = sys.argv[1:4]
now = int(time.time())
print(jwt.encode(
    {"iss": issuer, "iat": now, "exp": now + 900, "aud": "appstoreconnect-v1"},
    open(key_path).read(), algorithm="ES256", headers={"kid": kid}))
PY
)

base=https://appstoreconnect.apple.com/notary/v2/submissions
if (( $# >= 1 )); then
  id=$1
  echo "== submission $id =="
  curl -fsS -H "Authorization: Bearer $token" "$base/$id" \
    | python3 -m json.tool
  echo "== log (valid a few minutes) =="
  curl -fsS -H "Authorization: Bearer $token" "$base/$id/logs" \
    | python3 -c "import json,sys; print(json.load(sys.stdin)['data']['attributes']['developerLogUrl'])"
else
  curl -fsS -H "Authorization: Bearer $token" "$base?limit=10" \
    | python3 -c "
import json, sys
for s in json.load(sys.stdin)['data']:
    a = s['attributes']
    print(f\"{s['id']}  {a['createdDate']}  {a['status']:<12} {a['name']}\")"
fi
