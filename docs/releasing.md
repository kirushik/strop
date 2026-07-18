# Releasing, updating, and becoming an app

*Drafted 2026-07-18 for the 0.3.0 "non-product" round. Research grounding: four
deep-research passes (signing, update frameworks, Linux channels, update UX /
installers) on July 2026 state of the world; primary-source precedents: Zed's
release + auto-update pipeline (readable in our fork checkout), Obsidian's
update model, Scrivener's migration model, Flathub's Zed manifest. Reviewed
adversarially by Sol (read-only, 2026-07-18): 4 blockers, 7 majors — all
adjudicated and folded in; §14 records the round. Status: DECISION SHEET —
R1/R3/R6/R7/R9 sub-choices await Kirill; the rest are recommendations with
counterarguments recorded.*

## §0 Philosophy: the release surface is product surface

Everything in `design-principles.md` applies past the window's edge. The
updater is the sharpest test of P1 (*the tool never wants anything*) we have
yet faced: an auto-updater is the one component whose default posture, in
every mainstream app, is to want something — attention, a restart, a click.
Ours gets none of that. Concretely:

- **The updater never interrupts.** No toast, no badge on the writing surface,
  no restart prompt. Its entire visible existence is a status line inside
  About.
- **A new version is applied only across a launch boundary.** The binary the
  writer is typing into never changes under them.
- **The verb has an inverse** (P9): one previous binary is kept, and every
  format migration leaves a restorable original (§6) — the inverse covers the
  *documents*, not just the executable (see the rollback contract in §4).
- **The update machinery never touches user files.** Schema migration is the
  *document's* affair, governed by §6, and is consent-shaped, not
  update-shaped.

And one release-side principle: **no infrastructure**. GitHub (Releases CDN +
Actions) is the only server. Anything that requires a machine of ours to stay
up — TUF timestamp re-signing included — is out (see §4).

## §1 R1 — One identity, decided once

Windows AUMID, macOS `CFBundleIdentifier`, the Linux `.desktop` basename, the
Wayland `app_id`, and the Flathub app ID should all be **one reverse-DNS
string**, because dock grouping, file association, and store verification all
key on it, and two of the three platforms make it effectively immutable
(Flathub IDs are forever; changing a macOS bundle id orphans user defaults
and permissions).

There is a third candidate the first draft missed: **the storage identity
already exists.** `paths.rs` fixes `ProjectDirs` to `cc.pimenov.strop` —
domain-backed (pimenov.cc is ours), and Linux already normalizes the app
component to plain `strop` for XDG dirs. Options:

1. **`cc.pimenov.strop`** — costs nothing, Flathub-verifiable today via a
   `.well-known` file on pimenov.cc, and desktop/storage identity become one
   string with **zero migration**. Counterargument: it brands the app to a
   personal domain forever; if Strop ever deserves its own home, the Flathub
   ID won't follow it.
2. **Buy a project domain now** (`strop.dev` / `strop.app` / …, ~$20/yr) →
   `dev.strop.Strop`. Cleanest brand; requires deciding the storage-identity
   question below.
3. `io.github.kirushik.strop` — free, but couples the ID to GitHub instead of
   a domain we control; strictly dominated by option 1. Rejected.

**Storage identity (Sol F11):** whatever is chosen for the *desktop* identity,
`ProjectDirs` must either stay `cc.pimenov.strop` forever (documented as a
frozen legacy namespace) or move in the same commit. The installed base today
is Kirill plus a handful of testers, so aligning now is nearly free (a
one-time if-old-dir-exists move on macOS/Windows; Linux needs nothing) — and
it is the *last* moment that is true. Under option 1 the question dissolves.
If option 2 is chosen, the move gets a real contract before Phase A code:
old/new path pairs, both-exist conflict rule (old wins, new renamed aside),
idempotence, and a test — not a hand-waved "one-time move" (Sol, F11
concurrence condition).

**Recommend:** option 1 unless the project-domain itch is real, in which case
buy the domain *now* and align storage in the identity commit. **DECIDE
(Kirill).** In the same commit: Wayland `app_id` (currently `"strop"` in
`main.rs`) and the `.desktop` basename both become the chosen string.

## §2 R2 — Artifact shapes: ready-to-launch, per platform

### macOS: a real `.app` in a `.dmg` — non-negotiable

A bare Mach-O in a tar.gz fails every macOS requirement except "executes":
no Dock icon/name, no `Info.plist` → no `.strop` association, and it cannot
be notarization-stapled. Sequoia removed the right-click→Open bypass, so an
unsigned download is effectively locked for normal users.

- Build the bundle with a **hand-rolled CI script** (~60 lines: `Contents/
  {MacOS,Resources}`, `Info.plist` with `UTExportedTypeDeclarations` for
  `com.strop.document` / `.strop` + `CFBundleDocumentTypes` role Editor rank
  Owner, icns, binary). Zed maintains a cargo-bundle *fork* — the signal that
  upstream isn't sufficient; our needs are small enough to skip the tool
  entirely.
- `.dmg` via `hdiutil create -format UDZO` with the `/Applications` symlink.
- arm64-only, as today. Intel stays cut until someone asks; universal-by-lipo
  is a known recipe if they do.

### Windows: per-user Inno Setup installer + portable zip

- **Inno Setup, `PrivilegesRequired=lowest`**: installs to
  `{localappdata}\Programs\Strop`, Start-menu shortcut, `.strop` association
  (HKCU ProgId, `ChangesAssociations=yes`), Add/Remove entry. No UAC ever —
  which is also what lets the updater swap the binary without elevation.
  Inno over NSIS: same capabilities, saner language; over WiX/MSI: MSI's
  component/repair model actively fights self-updating apps; over MSIX:
  sideload requires a machine-trusted cert (admin once) and its own updater.
  Over Velopack's Setup.exe: see §4 — we don't take the framework, so we
  don't take its installer. (The ARP entry's displayed version is stamped by
  the installer and will drift as the exe self-updates; the updater rewrites
  the HKCU uninstall key's `DisplayVersion` as part of a successful swap.)
- **Portable zip stays** for the portable-app crowd, and **portable builds
  never self-update** (Sol F8): a self-replacing exe on a read-only or
  removable location is a bug farm, and a portable app that rewrites itself
  isn't portable. Distinct channel (`github-win-portable`), passive
  About-notice only. It writes no registry keys — associations come with the
  installer or by hand.
- x86_64 only, as today.

### Update payload contract (per channel)

What the updater downloads and applies is pinned here, not left to the CI's
imagination (Sol F9): **`github-win` = the signed `strop.exe` itself**
(single file — the whole reason §4 can be simple); **`github-mac` = a
`.tar.gz` of the stapled `Strop.app`** (not the DMG — DMGs are for humans);
all other channels have **no payload** (package managers or nothing).

### Linux: tarball + `.deb` + `.rpm` on the release; stores in §7

`cargo-deb` and `cargo-generate-rpm` (both actively maintained) run in the
existing Linux job and emit proper packages: binary, `.desktop`, hicolor
icons, MIME XML for `application/x-strop`, postinst `update-mime-database`/
`update-desktop-database`. These carry **no auto-update** (no repo behind
them) — they exist so "I use Debian, give me a real package" is answerable.
Glibc floor: ubuntu-22.04 (2.35) while the runner lives (deprecation starts
2026-09); `cargo-zigbuild --target x86_64-unknown-linux-gnu.2.28` is the
runner-proof successor **[UNVERIFIED for our gpui/wgpu/ashpd link chain —
needs one experimental CI run]**.

## §3 R3 — Signing: pay Apple, ask SignPath, attest everything

### macOS — do it, $99/yr

Apple Developer Program → Developer ID Application cert → hardened-runtime
`codesign` → `notarytool submit --wait` → `stapler staple`. All of it runs on
the `macos-latest` runner; **no Mac is ever needed** — the cert is created
with an OpenSSL CSR uploaded in the browser portal, and auth uses an App
Store Connect API key (scoped, revocable — Zed's exact pattern; their
workflow is copyable nearly verbatim). Fallback if we ever want to drop the
macOS runner: `rcodesign` signs *and* notarizes from Linux, but its last
release was Nov 2024 — reference, not foundation.

### Windows — SignPath Foundation first; unsigned is the honest fallback

Reality check from research: since 2024, **neither OV nor EV certificates
grant SmartScreen reputation** — reputation accrues per-file/publisher with
download volume regardless. So the expensive options buy less than folklore
says. The landscape for us:

- **Azure "Trusted Signing" (now Artifact Signing, $9.99/mo)** is closed to
  EU *individuals* — EU *businesses* only. Available iff Kirill registers a
  toiminimi and the Y-tunnus passes validation **[eligibility of EU sole
  traders: PARTIALLY UNVERIFIED — Microsoft FAQ vs community reports
  disagree]**. Its identity-bound reputation model is the only one that
  (aspirationally) skips per-file reputation — and it broke for everyone in
  March 2026 when Microsoft rotated the issuing CA. Not a silver bullet.
- **SignPath Foundation**: free for qualifying OSS; publisher string reads
  "SignPath Foundation" (not "Kirill Pimenov"); every release needs a manual
  approval click in their portal — which happens to rhyme with our
  human-publishes-the-draft-release ritual. Solo GPL projects qualify but
  approval is discretionary.
- **Certum OSS cert**: cheap, but smartcard/OTP workflows that cannot run
  headless on hosted runners. Not viable.
- **Peers**: Alacritty (wontfix'd signing), Helix, WezTerm ship Windows
  binaries **unsigned**. Only VC-funded Zed signs.

**Recommend:** apply to SignPath now (it's free and reversible); ship 0.3.0
unsigned-on-Windows if approval hasn't landed, exactly as peers do, with the
SmartScreen caveat in the release notes. Revisit toiminimi+Azure only if
Strop grows a commercial life of its own. **DECIDE (Kirill):** is the
"SignPath Foundation" publisher string acceptable? If it offends, the choice
collapses to unsigned-for-now.

### Both platforms — provenance attestations, immediately

`actions/attest-build-provenance` in release.yml: free, no identity
paperwork, Sigstore-backed SLSA provenance binding every artifact to this
repo + workflow. It's what security-conscious users and packagers actually
check in 2026 (Helix's exact posture). Attestations are audit evidence — the
updater does not consume them, and they do not repair the first-install
trust question (§4). They also answer the YubiKey idea:

**The detached-signature idea is retired.** Nobody verifies detached GPG
signatures; the trust anchor users can't get wrong is the updater's baked-in
minisign key (§4) plus attestations. The YubiKey keeps its existing jobs —
signing tags and publishing releases.

## §4 R4 — The updater: hand-rolled, minisign-signed, boring

### Why not the frameworks

- **Velopack** (the strongest framework candidate): unified installer+
  updater+deltas, drives signing. But (a) its packaging CLI drags a **.NET 8
  SDK** into all three CI legs; (b) its architecture — stub launcher,
  `current/` dir, `Update.exe` — is built for multi-file .NET/Electron
  trees; Strop is **one static binary**, the easy case that needs none of
  it; (c) `VelopackApp::build().run()` must own the head of `main()`,
  colliding with our single-instance rendezvous; (d) most damning: the
  update feed is **not framework-signed** — integrity is checksums in an
  unsigned manifest, authenticity delegated wholly to HTTPS + OS signing,
  and our Windows OS signing may be absent (§3).
- **Sparkle + WinSparkle**: the venerable EdDSA-appcast pattern, but two
  platform libraries, ObjC + C FFI, appcast XML — for functionality that is
  ~600 lines of Rust when the artifact is a single file.
- **cargo-packager/CrabNebula updater**: right security model (Tauri's
  minisign manifest), pure Rust, but brings a bundler we've already decided
  to hand-roll (§2) and no deltas anyway.
- **TUF**: disproportionate, deliberately. TUF's freeze-attack protection
  runs on a *timestamp role* whose key must re-sign expiring metadata
  perpetually — an online key we'd have to park in a GitHub secret, i.e.
  *more* attack surface for a solo project, not less. Its real adopters are
  PyPI/Docker-class registries; essentially no desktop app uses it. The
  "liveness checks" Kirill heard about are exactly this treadmill. We take
  the pragmatic subset: offline signing key, signed manifest with full
  target binding, host-restricted HTTPS, and client-side downgrade refusal.

### The manifest

`latest.json` + `latest.json.minisig`, assets of every release, fetched via
`https://github.com/kirushik/strop/releases/latest/download/latest.json` —
the CDN redirect path, **never** the REST API (60 req/hr per IP, shared
behind NATs). Signed contents (Sol F8 — the signature must bind everything
the client is about to trust):

- `product: "strop"`, `updater_protocol: 1`, `version`, `pub_date`,
  `notes_url` (restricted: client refuses anything but
  `https://github.com/kirushik/strop/…` — a signed URL is a navigation
  capability, so it gets a leash),
- per-target entries keyed by **channel + target triple + artifact kind**
  (`github-win/x86_64-pc-windows-msvc/exe`,
  `github-mac/aarch64-apple-darwin/app-tar`), each `{url, sha256, size}`.

The client requires **exact equality** with its compile-time channel,
triple, kind, and protocol before it downloads a byte. A mac tarball can no
longer be fed to a Windows exe path by a manifest-generation bug — the
signature would be attesting a tuple the client refuses.

Fetch discipline: the manifest and artifact fetches follow redirects only to
an allowlist (github.com + GitHub's CDN hosts); this is **host restriction,
not certificate pinning**, and the doc says so honestly (Sol security note).

### The key

A managed minisign key *set* (one key at a time in the normal case; two
during a rotation window). Public keys + their key IDs baked into the binary
(`minisign-verify` crate — pure Rust, tiny); unknown key IDs are refused.
Private key lives on Kirill's machine, passphrase-protected, backed up
offline, **never in CI** — CI produces everything except the one signature
(§12), so a compromised CI cannot feed updates to anyone. Routine rotation
(Sol's missed-bridge catch): the manifest carries **multiple signatures**
(`latest.json.minisig`, `.minisig2`) and a client accepts any signature from
its baked key list; a rotation **dual-signs for at least one full minor
series**, so clients that slept through the bridge release still update; the
key list in new binaries only grows until a retired key ages out, and
clients older than the whole dual-signing window are manual-reinstall
(stated, like the other residuals).

**Stated residual risks** (accepted, not hidden — Sol F7 adjudicated): (a)
*key compromise is unrecoverable in-band* — a stolen key can sign an
arbitrarily high version and dual-key rotation cannot revoke it for clients
that already trust it; recovery is a manual reinstall, announced loudly.
The mitigation is custody (offline, passphrase, backup), not machinery — a
signed key-epoch scheme is TUF creep, rejected for the same reasons TUF was.
(b) *Freeze*: an attacker who controls the network path can withhold
updates indefinitely; no expiring metadata means no detection. (c) *First
install*: the initial download (possibly unsigned on Windows) is trusted on
HTTPS + attestations alone; the baked key it carries is only as authentic as
that first fetch. All three are the honest price of "no infrastructure."
Client also persists the highest version it has ever *seen* and refuses
manifests below it (narrows replay; a user who intentionally reinstalls old
wipes state and that's fine).

### Staging (crash-consistent, Sol F12)

Two phases with different ownership (Sol N2 — the first revision's lock rule
contradicted itself). **Download phase, no lock**: fetch and verify into a
**uniquely-named temp dir owned by that check alone** (pid+random suffix), so
concurrent checkers cannot touch each other's bytes. **Publication phase,
under the update lock, briefly**: reconcile against the current manifest,
atomically promote the verified temp dir to `data_dir/updates/stage-<version>/`
+ its `ready` marker naming the exact artifact hash, and garbage-collect
every other stage and orphaned temp dir. All `ready`-marker writes, stage
promotion, GC, and apply happen under the lock; no network I/O ever does.
Partial downloads are invisible by construction; **a stage without its
matching `ready` marker does not exist** as far as apply is concerned;
marker/payload disagreement = delete the stage; one stage at a time,
size-capped; GC distinguishes live owned temp dirs (pid alive) from
orphans.

### The update lock (Sol F1 — the finding that reshaped this section)

Our single-instance rendezvous is **per-document** (`single_instance.rs`
keys the socket on the canonical file path), so it provides *zero* mutual
exclusion for an installation-wide binary swap — two launches for two
different documents happily run concurrently. Therefore: an **installation
update lock** (`data_dir/updates/lock`, OS advisory file lock, held only
during apply — never during download). Every mutation of stage / current /
previous state happens under it. A launch that finds the lock held skips
apply and runs the current binary — never waits, never races.

### Apply — strictly before rendezvous

Apply runs at process start, **before document resolution and before any
socket is claimed**. The swapping process holds no single-instance state at
all, so there is no handoff to get wrong — this ordering exists because the
Windows guard's named-pipe release is asynchronous (`single_instance.rs`
documents the up-to-a-second lag), which is exactly why apply-on-quit was
rejected: a quit-time swap + immediate relaunch would rendezvous against
the dying listener and exit as "already open" (Sol F2). Sequence:

1. Take the update lock (non-blocking; on failure, launch normally).
2. Re-verify the staged artifact against its `ready` marker.
3. Swap (per-OS below), journal each phase durably.
4. Release lock, spawn the new binary with original argv, exit. The child
   performs document rendezvous exactly like any fresh launch.

- **Windows**: `self-replace` crate (the rename dance a running exe needs);
  old exe kept as `strop-prev.exe`; ARP `DisplayVersion` rewritten. Other
  running instances keep executing their old image (Windows keeps the
  renamed file's image alive) and pick the new binary up at their next
  launch.
- **macOS** (Sol F3/N1 — "atomic rename" was a fiction, and a naive
  two-rename journal has a crash window where `Strop.app` *doesn't exist*
  and the recovery code lives inside the missing bundle): the staged `.app`
  is first **copied to a sibling temp dir on the destination volume**
  (`Strop.app.staged` next to `Strop.app` — same filesystem, no EXDEV) and
  re-verified there. The swap itself is **`renamex_np(RENAME_SWAP)`** — the
  atomic directory-entry *exchange* APFS provides and Sparkle relies on:
  one syscall, after which `Strop.app` is the new version and `.staged`
  holds the old bundle. **A valid `Strop.app` exists at every durable
  boundary**; there is no state in which the Dock icon points at nothing.
  Then journal, rename old aside as `Strop.app.previous`, journal `done`.
  Recovery at launch only ever relabels leftovers (`.staged` present +
  journal pre-swap → discard by re-verification; post-swap → finish the
  aside rename). Exchanging the bundle out from under the running process
  is safe (the image is inode-backed). If `renamex_np` fails (non-APFS
  volume): no clever fallback — calm About line, manual update, done. If
  `/Applications` isn't writable (IT-managed Macs): same — no elevation
  attempt. The bundle CI ships is already signed+notarized+stapled, so the
  swapped-in app verifies offline; our process sets no quarantine flags on
  files it writes.
- Any verification or permission failure: run the current binary, one About
  line, delete the stage. Never block launch.

### Rollback — an honest inverse (Sol F4)

`strop --rollback` re-instates the kept previous binary. But a binary
rollback is a *lie* if the new version has already migrated documents the
old one now refuses (`store.rs` refuse-newer is a hard wall, correctly).
The contract that keeps the inverse honest:

1. **Schema version never changes on a patch release.** Auto-updates within
   a minor series (0.3.0 → 0.3.4) are therefore *always* rollback-safe.
   This is a law for release planning, recorded here.
2. Crossing a schema-bumping release, rollback pairs with **document
   restore**: the migration ledger (§6) maps every migrated file to its
   pristine backup, and About's migration view offers per-document restore.
   `--rollback` across a schema bump says so instead of pretending.

### Channel gating & config

Build-time `option_env!("STROP_DIST_CHANNEL")` bakes `github-win` /
`github-mac` / `github-win-portable` / `github-linux` / `flathub` / `deb` /
… into the binary. Self-update activates **only** for `github-win` /
`github-mac`. Linux and portable builds never self-modify — package
managers own the former; the latter gets a passive "0.3.2 is out" line in
About. Unset channel (a local `cargo build`) = updater fully inert.
Config: `[update] check = true|false`. `false` kills even the check — the
GitHub fetch is the only network call this app ever makes, and it's
disable-able; worth a proud sentence in the README.

### Tests are not optional

Zed shipped a post-mortem where a settings-parse regression silently
disabled auto-update for two releases. We test: manifest parse (absent/
garbage config included), target-tuple mismatch refusal, downgrade and
unknown-key refusal, staged-swap fault injection at every journal phase
(kill, restart, assert the recovery table), **concurrent launches during a
staged update** (two documents, both racing the lock — Sol F1's scenario),
and a rig scenario running old binary → stage → relaunch → assert swap,
plus real-Windows CI coverage for the pipe/self-replace path.

## §5 R5 — Update UX: silence, with one honest window

Kirill's instinct ("silent 'we will update on next launch', interactive
checker in About") survives research contact almost unmodified — it is
Obsidian's model, and the HCI literature (NN/g on system-initiated
interruptions; the 2009 Chrome silent-update study; 2024 survey: 30% of
users know what an update changed) supports both halves: don't interrupt,
*and* don't be unaccountable. So:

- **Mid-session**: nothing. Ever.
- **About** is the updater's single surface: current version + channel,
  "checked 2 h ago", "0.3.1 downloaded — next launch gets it", a manual
  check button, a link to what changed. All states, one calm place
  (P10: the control is the indicator).
- **After an update**: no popup, no auto-opened changelog. About holds an
  "updated 0.3.0 → 0.3.1 · what changed" line for the first session.
  Anything richer (a "what's new" page in-app, feature opt-in tours) goes to
  the system-voice/chrome backlog where Kirill already parked its siblings.
- **Never nag on failure** either: a broken update is an About line
  ("couldn't apply 0.3.1 — kept 0.3.0"), not a dialog.

## §6 R6 — The draft is sacred: format compatibility as a tested contract

`store.rs` already has the right bones: `SCHEMA_VERSION_KEY`, refuse-newer
with a clear message, a migration `match`. What 0.3.0 adds:

1. **Backup at open, not at save** (Sol F5 — the first draft's
   backup-on-first-save was a hole: `Store::open` already performs open-time
   transformations, so by save time the pristine bytes are gone from
   memory). The contract: when opening a file whose schema < current, the
   **exact bytes just read** are written to
   `data_dir/migration-backups/<full-blake3>.strop` (full hash — no prefix
   collisions), fsynced, and recorded in a ledger line (`when, source path,
   source schema, hash`) **before any migrated save can be prepared** — and
   the ledger commit is itself a crash-consistent transaction (Sol N3):
   append + fsync the ledger *and* fsync its parent directory before the
   save-block clears, so no crash can leave a migrated document without its
   About mapping. Fault-injection tests sit between every backup / ledger /
   save boundary. The bytes are in hand at open by definition. If the backup
   or ledger write fails, the document opens normally but **saving is
   refused** (with the reason) until both land — the pristine original on
   disk stays the backup of record. Idempotent by hash; kept forever (they're small;
   asset-GC does not apply). About gains a migration view: per-document
   restore (the F4 pairing), not filesystem archaeology.
2. **A corpus, in CI — asserting semantics, not prose** (Sol F6: a
   text-only round-trip would pass while silently dropping checkpoints,
   anchors, or provenance). `tests/corpus/` holds real `.strop` files
   frozen at each released minor. For each fixture, CI asserts a **semantic
   projection covering every durable subsystem**: text + formatting spans +
   block kinds, notes with anchor positions, checkpoint list *and* a
   materialized checkpoint state, journal, graveyard, asset references,
   provenance — then performs a deterministic post-migration edit, saves,
   reopens, and asserts again (CRDT snapshots are not byte-comparable;
   editability after migration is part of the contract).
3. **Fixtures are frozen before the tag** (Sol F10): the corpus file for
   release N is generated from N's release candidate and merged *before*
   `vN` is tagged, so the tag's own CI runs it. A fixture added after
   tagging protects nobody. The ritual (§12) verifies presence, never
   creates.
4. **Refuse-newer stays a hard wall** — a 0.3.0 binary must *never*
   half-open a 0.4.0 file. (The first draft floated a `min_supported_schema`
   lever in the manifest — cut: no client can inventory every document a
   user might open later, so gating updates on it is theater. Sol,
   concurring.)
5. Migration itself is atomic (temp file + rename), which the save path
   already guarantees.

## §7 R7 — Linux channels: Flathub-first; OBS is a hobby, not a duty

The ex-SUSE heart will not love this section. Findings, then the sting:

- **Flathub is the primary channel.** It's the one Linux channel with true
  install-once-stays-fresh for GUI-store users, and its maintenance cost is
  near zero: the external-data-checker bot PRs each new GitHub release into
  the manifest (tag → bot PR → merge → published). Two tricks make it cheap
  for us specifically: (a) **repackage our release tarball** instead of
  building Rust in the sandbox — Zed's community manifest does exactly this,
  Flathub permits it for upstream-verified apps, and it sidesteps the whole
  flatpak-cargo-generator-vs-git-fork-dependency fight; (b) because Strop
  already speaks ashpd/portals, we can ship **portal-only — no
  `--filesystem=home`** — a *cleaner sandbox than Zed itself* (they need
  `--filesystem=home` + a sandbox-escape wrapper). Sharp edge to design
  around: the document portal grants access by handle, so **recent files
  must persist document-portal paths, not host paths**, in the Flatpak
  build — this touches the recents/visits code and wants its own small
  round. GPU: `--device=dri` on the 25.08 runtime; NVIDIA-proprietary is the
  known fragile row (same risk Zed carries).
- **AUR**: publish a source PKGBUILD (Arch allows our git-pinned fork
  directly); a `-bin` variant is optional polish. Low effort, self-selecting
  audience.
- **Nix**: in-repo `flake.nix` with `cargoLock` + `allowBuiltinFetchGit`
  (fine for a flake, forbidden in nixpkgs); let the community carry it into
  nixpkgs — noting the known `importCargoLock` trap when fork and upstream
  share a crate name+version (#183344) for whoever does.
- **Snap: skip.** Structural, not aesthetic: the mesa-2404 content-snap can
  skew against host drivers (a Vulkan crash axis we'd own), updates are
  forced on users (anti-P1), `removable-media` isn't auto-connected (a
  writer's USB stick doesn't open), and the portal-cgroup misdetection we
  personally debugged is a live, unfixed failure mode of snap confinement.
- **OBS**: honest assessment — it duplicates Flathub's reach at a
  multiple of its maintenance cost: `.spec` + `debian/` in parallel, a
  cargo-vendor service whose `--locked`-fidelity against our git-pinned
  fork is exactly the kind of thing that breaks quietly **[UNVERIFIED —
  needs an experimental run]**, and an ABI matrix (wayland/vulkan/
  fontconfig across distro versions) that Flatpak exists to abolish. If it
  happens, it happens as nostalgia on a rainy weekend, not as release
  infrastructure. **DECIDE (Kirill)** — the counterargument to my
  counterargument: OBS gives *native packages with real auto-update* to
  zypper/dnf users, which Flathub never will, and you know the toolchain
  cold.
- **Debian/Fedora official: don't attempt** (git-forked gpui is
  policy-impossible in Debian, painful in Fedora; even Zed doesn't try).
- **AppImage: later, if asked.** Viable but carries the
  exclude-host-graphics-libs tax (libvulkan/libwayland/libstdc++ bundling is
  the canonical Vulkan-AppImage failure) and no one recommends it as primary
  in 2026.

## §8 R8 — About: the app's colophon

A second fixed-size gpui window (the keymap window is the pattern). Strop is
a book-shaped tool; its About is a **colophon**, not a corporate box:

- Wordmark + icon, version, commit, channel, build date.
- The GPL notice trio (copyright, no-warranty, "free software — you may
  redistribute under GPL-3.0-or-later"), link to COPYING and the repo. The
  GUI About box is the GPL's own suggested vehicle for this.
- **Third-party licenses**: `cargo-about` (Zed uses it; actively maintained)
  generates the full attribution document at build time; embedded and opened
  as a plain scrollable text view in-app. CI gets a freshness check so the
  embedded list can't drift from Cargo.lock.
- The updater's one surface (§5) and the migration-backups view (§6).
- Typeset as an actual colophon — "Set in URW Bookman" and friends. That
  line *is* the easter egg's front door; the egg itself (this is that kind
  of project) should be small, writerly, and designed in its own moment —
  one candidate to beat: the version string, clicked seven times, hones —
  the About border acquiring a razor's polished edge, GPU shader and all.
  Parked in the chrome backlog; reserved here so About's layout leaves it
  room.

## §9 R9 — The mark

Strop already owns a designed icon language (ten bespoke SVGs, two
families, documented in `iconography.md`). The mark should be **grown from
that garden, not commissioned around it**: a strop — the leather band that
keeps an edge keen — reduced to a single bold silhouette, drawn in the same
stroke/corner grammar, validated at 16/32/256/1024 px. Process: SVG
candidates iterated in-repo against the visual rig, Kirill as art director.

Honesty clause: AI-drafted artwork is (per the US Copyright Office's 2025
report) not copyrightable as such — thin protection for the mark itself.
For a GPL project this is mostly moot (the mark can still be
trademark-registered if that ever matters, and nothing else in the project
leans on artwork copyright), but if a legally-ownable mark matters, a human
commission ($300–1000, Dribbble/Fiverr tier) is the alternative — best done
*after* an AI-drafted round has fixed the brief cheaply. **DECIDE (Kirill),**
though drafting candidates costs nothing and forecloses nothing.

Pipeline (pure Rust, an xtask): master SVG → `resvg` rasterize → `ico` +
`icns` crates + hicolor PNG set + scalable SVG; `.ico` embedded in the exe
via `embed-resource`; icns into the bundle; hicolor + `.desktop` +
metainfo into deb/rpm/Flatpak.

## §10 R10 — Platform citizenship (mostly Phase D)

- **Now (free or forced):** Wayland `app_id` == `.desktop` basename == the
  §1 identity (this alone gets single-icon multi-window grouping on
  GNOME/KDE — `StartupWMClass` is X11-only and ignored on Wayland); verify
  our fork carries Zed's fix for `app_id` being sent late on first commit
  (upstream PR #55583) **[UNVERIFIED — check during Flathub round]**; file
  associations ride the installers (§2).
- **Phase D:** Windows `SetCurrentProcessExplicitAppUserModelID` + the same
  AUMID stamped on the Start-menu shortcut, `SHAddToRecentDocs` per
  open/save (JumpList recents for ~free once the ProgId exists); macOS
  `NSDocumentController.noteNewRecentDocumentURL` (feeds Open Recent *and*
  the Dock menu, no NSDocument adoption needed — objc2 FFI); Linux
  `.desktop` `Actions=NewWindow;`, recents via `recently-used.xbel`.
- **Declined:** Unity LauncherEntry badges/progress (a writing tool with a
  dock badge is the tool wanting something); MSIX-gated Windows
  share-targets/widgets (needs package identity we don't carry); macOS App
  Intents (needs a Swift shim; revisit only if Shortcuts users ask).
- Single-instance stays on the socket rendezvous (portable, proven); D-Bus
  activation (`org.freedesktop.Application`) is a possible later refinement
  for GNOME/KDE open-file routing, not a correctness need.

## §11 The phases, reordered

Kirill's phase sketch had installer-and-artifact questions (his Phase 3) after
updater questions (his Phases 1–2). The dependency runs the other way: **the
updater updates an installed thing**, so the artifact/identity/installer layer
is the floor. Also his Phase 0 hunch was right — and bigger than sketched,
because the icon and the `.app` bundle both live there too.

**Phase A — Become an app** *(0.3.0 blockers)*
Identity (§1), including the storage-identity call. Icon v1 (§9). macOS
bundle+DMG, signed+notarized (§2/§3). Windows Inno installer + portable zip;
SignPath application submitted (§2/§3). deb+rpm. About window incl. licenses
+ GPL notices (§8). Provenance attestations. release.yml grows the
bundling/signing legs and the manifest job (§12). Corpus seeded per §6.

**Phase B — Self-updating** *(also 0.3.0 — the promoted binary must already
know how to leave)*
Manifest + minisign keypair + the local sign-and-verify script (§12).
`strop-update` module: check / stage / update-lock / journaled swap /
recovery table / rollback. Channel gating + `[update]` config. The §4 test
battery — fault injection and the concurrent-launch race included.

**Phase C — Meet the package managers** *(0.3.x fast-follows, independent)*
Flathub (manifest repo, portal-only, x-checker bot, verified badge) —
including the portal-persistent recents fix. AUR PKGBUILD. `flake.nix`.
OBS iff Kirill overrules §7. AppImage iff users ask.

**Phase D — Citizenship & chrome** *(backlog, with the other design-heavy
chrome)*
Recents/JumpLists/Dock menu (§10). D-Bus activation. "What's new" surface.
The easter egg (§8). Any staged-rollout ambitions (which would need the
infra we're refusing — so, likely never).

## §12 The release pipeline and the ritual (once A+B land)

**The DAG, fixed here so CI can't improvise it** (Sol F9 — hashes must be
taken from *final* bytes, and every ordering below exists because some step
mutates the artifact): per platform — build with channel baked → sign the
inner executable (SignPath / codesign) → construct bundle / installer /
archives → notarize + staple (macOS) → the resulting files are the
**immutable final payloads** → upload as draft assets → a final CI job
downloads the draft assets back, computes hashes, and generates
`latest.json` with deterministic URLs (`releases/download/vX.Y.Z/<name>` —
constructible before publish; the `latest/` redirect only has to work
*after* publish, which is when clients look) → attestations over the same
final assets.

**The ritual:**

1. Freeze the corpus fixture for this release and merge it (§6.3). Tag
   `vX.Y.Z` (signed, as ever). CI runs the DAG; draft release assembled;
   the tag's CI ran the full corpus by construction.
2. Kirill runs the **local verifier script** (part of Phase B, not a
   by-hand checklist — Sol's ritual notes, all of them, land here): it
   downloads every draft asset via authenticated `gh`, recomputes hashes
   and sizes *from those downloaded bytes* (independent of CI's claims),
   checks staple/signature validity where applicable, regenerates
   `latest.json` and diffs it against CI's, then displays version, tag
   commit, channels, triples, artifact names, and hashes for explicit
   confirmation — and only then signs those exact bytes with minisign and
   uploads the `.minisig`. It refuses to leave a state where manifest and
   signature could diverge.
3. The same script gates **publish**: it re-checks that `latest.json` +
   `.minisig` are present, valid, and reference every required asset (and
   no stray), then publishes the release. Abort paths are first-class:
   wrong signature / missing asset / notarization stuck / SignPath approval
   pending each leave the draft intact with a named next step — a draft is
   always safe to delete and re-cut.
4. **Withdrawal policy**: a bad release is never unpublished into a hole —
   it is *superseded* by publishing a fixed higher version (clients only
   move forward; §4's monotonicity makes yanking-by-deletion useless
   anyway). Note in the release log; the key fingerprint and signing-tool
   version ride every log entry.
5. Post-publish, the *separate* non-integrity checklist: merge the Flathub
   bot's PR when it arrives; AUR bump if no community maintainer beat us.
6. Fleet behavior: from 0.3.0 on, next-launch updates within ~a day of
   publish. (There is no fleet before 0.3.0 — it is the first
   self-updating release.)

## §13 Open questions & unverified flags

**For Kirill:** §1 identity (cc.pimenov.strop vs new project domain) + the
storage-alignment call; §3 SignPath publisher-string tolerance; §6 backup
location taste (data_dir + About view, per Sol, is the draft's answer —
overrule if you want siblings); §7 OBS as hobby-or-not; §9 AI-drafted vs
commissioned mark.

**To verify by doing (each is one small experiment):** cargo-zigbuild vs our
link chain (§2); EU sole-trader eligibility for Azure signing, only if the
toiminimi path ever activates (§3); OBS cargo-vendor fidelity, only if §7 is
overruled; fork's Wayland `app_id`-timing fix (§10).

## §14 Review ledger

- **2026-07-18, Sol (GPT-5.6), read-only adversarial review**: 4 blockers,
  7 majors, 1 minor. Accepted with design changes: F1 update lock (the
  per-document socket provides no installation-wide exclusion), F2
  apply-before-rendezvous ordering (async pipe release), F3 macOS journaled
  state machine replacing "atomic rename", F4 rollback/schema contract
  (patch releases never bump schema; ledger-backed document restore), F5
  backup-at-open (save-time backup captured post-migration bytes), F6
  semantic-projection corpus, F8 manifest target binding + portable-never-
  updates + bootstrap honesty, F9 release DAG + final-bytes hashing, F10
  corpus-before-tag, F11 storage-identity surfacing (which un-earthed
  cc.pimenov.strop as an identity candidate), F12 staging protocol.
  Attenuated: F7 — key IDs + custody + stated residual instead of a signed
  key-epoch scheme (TUF creep). Cut on Sol's concurrence: the
  `min_supported_schema` manifest lever (uninventoriable, therefore
  theater).
- **2026-07-18, Sol, concurrence round on the revision**: overall *dissent*
  with three findings, all folded in: N1 (blocker) — the two-rename macOS
  journal could crash into a state with no `Strop.app` and the recovery
  code inside the missing bundle → replaced with `renamex_np(RENAME_SWAP)`
  atomic exchange (Sparkle's approach; a valid bundle exists at every
  durable boundary, no helper binary needed); N2 — the lock/staging rules
  contradicted each other → split into lock-free download-to-owned-temp +
  brief-lock publication/GC/apply; N3 — ledger durability now a
  crash-consistent transaction (fsync file + parent dir before the
  save-block clears). Plus the missed-bridge rotation hole → multi-signature
  manifests, dual-signing for a full minor series. Sol *concurred* with both
  CEO overrules (F7 attenuation, `min_supported_schema` cut).
- **2026-07-18, Sol, final ack round**: overall **concur** — "I would now
  build this as specified." N1/N3 resolve outright; N2 and rotation resolve
  with two editorial notes, both folded (stage-requires-marker made
  explicit + live-temp-dir GC rule; "one keypair" → "managed key set").
  Design review CLOSED; implementation may reference this doc as the
  agreed spec.
