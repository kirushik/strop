# Flathub packaging

Strop's Flathub build **repackages the flathub-channel tarball** from the
tagged GitHub release (docs/releasing.md §7) — no Rust builds in the
sandbox, no cargo-vendor fights with the git-pinned gpui fork. The
tarball is produced and attested by tagged CI (`release.yml`, linux job),
already laid out as the `/app` prefix, and stamped
`STROP_DIST_CHANNEL=flathub`, so About names the channel honestly and the
updater stays passive ("… arrives through your package manager").

The sandbox is **portal-only**: no `--filesystem` at all. Known sharp
edge, accepted for now: paths granted through the document portal are
`/run/user/…/doc/…` handles, and the recents/visits code stores what it
sees — cross-restart recents inside Flatpak want their own small round
(spec §7 records this).

**The glibc law.** A repackaged binary must be built against a glibc no
newer than the runtime's. CI builds on ubuntu-22.04 (glibc 2.35), well
under the 25.08 runtime — safe. A binary built on a rolling host may
want a newer glibc and die at launch with `GLIBC_x.yy not found`
(observed 2026-07-23 with a host build). If the release runner image is
ever bumped, re-check against the runtime before shipping; when the
runtime branch is bumped instead, the floor only rises.

## Files

- `cc.pimenov.strop.yml` — the manifest, source of truth. The Flathub
  repo carries a copy of it.
- `flathub.json` — restricts Flathub's buildbot to x86_64 (the only arch
  we publish a Linux binary for).

## Steady state (per release, after the first submission)

Nothing manual. The manifest's `x-checker-data` watches
`releases/latest` (drafts and prereleases are invisible to it, so the
bot can never get ahead of the signing ritual). Within hours of
publishing vX.Y.Z, flathub-external-data-checker opens a PR on
`flathub/cc.pimenov.strop` bumping url + sha256; merging it publishes
the update (ritual §12.5). Optionally run `script/flathub-pin.sh X.Y.Z`
to keep the in-repo copy in sync.

## First submission (one-time, after v0.3.1 publishes)

1. `script/flathub-pin.sh 0.3.1` — verifies the published asset's
   provenance attestation and pins url + sha256 in the manifest here.
2. Fork `flathub/flathub`, create a branch **off the `new-pr` branch**,
   add `cc.pimenov.strop.yml` + `flathub.json` (flat, no subdirectory),
   open a PR against `new-pr`.
3. The buildbot builds it; address linter findings
   (`flatpak run --command=flatpak-builder-lint org.flatpak.Builder
   manifest cc.pimenov.strop.yml` reproduces them locally).
4. After merge, Flathub creates `flathub/cc.pimenov.strop` and invites
   the GitHub account as collaborator — accept, that's where the bot's
   PRs arrive.
5. **Verification** (the checkmark + "install from the developer"
   badge): on the Flathub website, claim the app; the app id
   `cc.pimenov.strop` verifies via the `pimenov.cc` domain — a token at
   `https://pimenov.cc/.well-known/org.flathub.VerifiedApps.txt`.

## Local build

CI cannot build this manifest before a release exists (the source is a
published asset). To test locally, build the tarball the way CI does,
then point a copy of the manifest at it:

```bash
STROP_DIST_CHANNEL=flathub cargo build --profile dist --locked -p strop-app
# stage the tarball as release.yml's linux job does, then in a scratch dir:
#   sources: [{type: archive, path: /path/to/strop-VER-x86_64-unknown-linux-gnu-flathub.tar.gz, sha256: …}]
flatpak run org.flatpak.Builder --user --install --force-clean build-dir manifest.yml
flatpak run cc.pimenov.strop
```
