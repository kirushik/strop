# macOS signing & notarization — the Mac-free runbook

Strop's `release.yml` `mac` job already signs, notarizes, and staples; it is
dormant behind the `MACOS_SIGNING_ENABLED` repository variable. This is the
one-time setup that lights it up. **No Mac is needed** — the certificate is
minted from an OpenSSL CSR in the browser portal, and notarization uses an App
Store Connect API key. Everything below runs on Linux.

## 0. Prerequisites

- Apple Developer Program membership **Active** (payment alone is not enough —
  check developer.apple.com/account → Membership).
- **Program License Agreement accepted** in the portal, or the Certificates
  page stays empty.
- You are the Account Holder (Individual/Sole Proprietor enrolment — you are).

## 1. Developer ID Application certificate (→ `MACOS_CERT_P12`)

Generate a keypair + CSR on Linux. Apple sets the real certificate subject from
your account identity; the CSR subject fields are largely cosmetic.

```bash
openssl genrsa -out developerID.key 2048
openssl req -new -sha256 -key developerID.key -out developerID.csr \
  -subj "/emailAddress=apple@pimenov.cc/CN=Strop Developer ID/C=DE"
```

In the portal → **Certificates, Identifiers & Profiles → Certificates → +** →
**Developer ID Application** → upload `developerID.csr` → download
`developerID.cer` (DER).

Bundle the leaf certificate and its private key into a password-protected
PKCS#12:

```bash
openssl x509 -inform DER -in developerID.cer -out developerID.pem
openssl pkcs12 -export -name "Developer ID Application" \
  -inkey developerID.key -in developerID.pem \
  -out developerID.p12 -passout pass:CHOOSE_A_STRONG_PASSWORD

base64 -w0 developerID.p12   # → paste as the MACOS_CERT_P12 secret
```

- `MACOS_CERT_P12` = the base64 blob above.
- `MACOS_CERT_P12_PASSWORD` = the password you chose.

The Apple *Developer ID Certification Authority* intermediate is present on the
`macos-latest` runner, so a leaf+key `.p12` is sufficient for the chain. (If a
future runner ever lacks it, append the intermediate PEM into the `.p12`.)
Developer ID certs are valid ~5 years — this is not an every-release chore.

## 2. App Store Connect API key for notarization (→ `APPLE_API_KEY*`)

Portal → **App Store Connect → Users and Access → Integrations → Keys → App
Store Connect API** → **Generate API Key** (labelled "Team Keys" in older docs;
Apple renamed it). A fresh account shows **Request Access** first — as the
Account Holder you self-approve instantly, then the Generate button appears.
Least privilege that notarizes is the **Developer** role.

- **Download the `AuthKey_XXXXXXXXXX.p8` — once only**, Apple never shows it again.
- Note the **Key ID** (10 chars, in the key row).
- Note the **Issuer ID** (UUID at the top of the Keys page — shared by all keys).

```bash
base64 -w0 AuthKey_XXXXXXXXXX.p8   # → paste as the APPLE_API_KEY secret
```

- `APPLE_API_KEY` = the base64 of the `.p8` (`bundle-mac.sh` decodes it to a
  temp file, since `notarytool --key` wants a path).
- `APPLE_API_KEY_ID` = the Key ID.
- `APPLE_API_KEY_ISSUER` = the Issuer UUID.

## 3. Wire the repo

Set **Secrets** (Settings → Secrets and variables → Actions → Secrets):

| Secret | Value |
| --- | --- |
| `MACOS_CERT_P12` | base64 of `developerID.p12` |
| `MACOS_CERT_P12_PASSWORD` | the `.p12` password |
| `APPLE_API_KEY` | base64 of the `.p8` |
| `APPLE_API_KEY_ID` | the 10-char Key ID |
| `APPLE_API_KEY_ISSUER` | the Issuer UUID |

Set the **Variable** (Variables tab): `MACOS_SIGNING_ENABLED = true`.

## 4. Verify

Trigger the workflow (`workflow_dispatch` **on the branch that carries the new
`release.yml`** — a dispatch on a branch without it runs whatever old workflow
lives there — or push a tag). The `mac` job should import the identity,
`codesign --options runtime`, `notarytool submit --wait` (a few minutes), and
`stapler staple`. The job then verifies its own product on the runner — the
maintainer has no Mac, so a green job IS the proof of Gatekeeper acceptance:

```bash
codesign --verify --deep --strict --verbose=2 Strop.app
spctl -a -vv -t install Strop.app         # → "accepted, source=Notarized Developer ID"
xcrun stapler validate Strop.app
```

(These run in `bundle-mac.sh` after stapling; an unusable certificate also
fails the job at the import step rather than falling through to an ad-hoc
signature.)

If notarization is rejected, `notarytool log <submission-id>` (in the run output)
names the exact offending binary/entitlement.

## Rotation & revocation

- **Revoke** a leaked cert in the portal; revoke a leaked API key on the Keys
  page. Both are independently revocable without touching the other.
- Already-notarized, stapled builds keep working after cert expiry — the
  notarization ticket is what Gatekeeper checks, and it is stapled into the
  artifact.
