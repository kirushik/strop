# Flathub packaging

Strop's Flathub build **repackages the flathub-channel tarball** from the
tagged GitHub release (docs/releasing.md §7) — no Rust builds in the
sandbox, no cargo-vendor fights with the git-pinned gpui fork. The
tarball is produced and attested by tagged CI (`release.yml`, linux job),
already laid out as the `/app` prefix, and stamped
`STROP_DIST_CHANNEL=flathub`, so About names the channel honestly and the
updater stays passive ("… arrives through your package manager").

**The sandbox.** Files the writer *opens* travel through portals only —
there is no `--filesystem=home`, and no blanket host access. The single
static grant is `--filesystem=xdg-documents`, and it exists for the
other direction: documents Strop *mints* (first-launch welcome, `--new`,
the sibling fallback in `host_parent_or_documents`) are born in the
documents folder. Inside the sandbox `$HOME` is a per-instance tmpfs
where `create_dir_all` and every autosave **succeed** and then evaporate
at quit — verified 2026-07-26 by writing a file in one instance and
finding it gone from the next. That is silent data loss on the default
first-launch path, so the grant is not optional; §7's red line is
`--filesystem=home`, which this is not.

Portal paths themselves are durable: grants live in
`~/.local/share/flatpak/db/documents` on the host, so a
`/run/user/…/doc/…` path in recents survives restarts and reboots
(checked with `flatpak documents`). An earlier draft of this file
claimed otherwise — it was wrong.

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
3. The buildbot builds it; address linter findings. Locally,
   `flatpak run --command=flatpak-builder-lint org.flatpak.Builder
   manifest cc.pimenov.strop.yml` covers only the manifest — the
   appstream/desktop/screenshot checks that Flathub actually rejects on
   need a built tree: `… flatpak-builder-lint builddir build-dir`. That
   run reports `appstream-external-screenshot-url` locally; Flathub's
   buildbot passes `--mirror-screenshots-url`, which clears it.
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
