# File compatibility

> A document Strop was asked to open must open, or say why.
> There is no third outcome.

The blank page is not a neutral failure. A writer who sees one believes their
work is gone, and the belief arrives before any explanation can. Every rule
below exists to make that page unreachable.

## 1. The field report that wrote this document

2026-07-24. A `.strop` file was opened from the desktop with "0.3.0 installed
from the deb package" and came up **empty**, while the same file opened
correctly in a build from source. The file was 81 KB and intact.

The file format had nothing to do with it.

**What was actually running.** Both a deb and a Flathub build of 0.3.0 were
installed. Flatpak's export directory precedes `/usr/share` in
`XDG_DATA_DIRS`, so `xdg-mime query default application/x-strop` answers with
the Flatpak's `cc.pimenov.strop.desktop`. A double-click ran the **Flatpak**.
Both builds call themselves 0.3.0, so there was nothing to notice.

**Why it was empty.** The Flathub manifest is portal-only — deliberately no
`--filesystem` grant. A file opened from the file manager therefore arrives as
`/run/user/1000/doc/<id>/name.strop`, which the sandbox **can** read.
`files::resolve_portal_path` then asked the portal for the host path, got
`/home/…/Documents/Strop/name.strop`, and returned it — a path that does not
exist inside the sandbox. `Store::open` read a missing file, and a missing
file is how Strop spells *brand-new blank document*. No error was possible,
because nothing had failed: the birth path and the failure path were the same
path.

**Why it was worse than a blank page.** `/home/…/Documents` is *creatable*
inside the sandbox, as a per-instance tmpfs. Typing into that blank page would
have autosaved, successfully, into a directory that evaporates at quit. The
original file would have been untouched, and the morning gone.

Measured, both at 0.3.0, same file:

| build | `doc_chars` | `manuscript_words` |
|---|---|---|
| deb (`/usr/bin/strop`) | 3858 | 640 |
| Flathub (`flatpak run`) | **0** | **0** |

**What was ruled out, with evidence.** `strop-core` is byte-identical between
`v0.3.0` and `HEAD` — the entire durable layer. The only dependency change was
loro 1.13.6 → 1.13.7, and a probe built against each version read every one of
the author's real `.strop` files to identical text, spans and schema. Loro's
own diff confirms it: `encoding/fast_snapshot.rs`, which owns the on-wire
layout, is unchanged between those releases.

## 2. What changed

**The resolver stops lying about reach** (`files.rs`). Two independent arms,
because one of them must keep holding when the other's diagnosis is wrong:

- *Named:* a sandboxed process never resolves a portal path. Inside a
  sandbox the doc-portal path **is** the document's address. Confinement is
  read from `/.flatpak-info` at runtime, never baked — the same binary is
  confined from Flathub and unconfined from the deb.
- *Unnamed:* a portal path that **opens** is never traded for a host path that
  does not. Every confinement we have not learned to name lands here.

**Birth stops being the fallthrough** (`main.rs`). A path the writer *picked*
— from the file manager, the file chooser, the recents list — names a document
that already exists, so a miss is now reported rather than born blank. A path
they *typed* keeps the opposite rule: `strop notes/new-essay.strop` is still
how you start one. The distinction is decided on the path **as it arrived**,
before resolution, so the resolver cannot launder the promise away.

**Loro's verdict reaches the writer in their language** (`store.rs`). Loro
states its policy in one line — *"backward compatible but not forward
compatible"* — and hands an older build meeting a newer encode mode an
`IncompatibleFutureEncodingError`. That now surfaces as "written by a newer
version of Strop" rather than "damaged": one points at the download, the other
at the backup, and they must not be confused.

## 3. The reservation: two numbers, not one

`.strop` is a Loro snapshot. `meta.schema_version` says what the **writer**
used — but it is read *after* `doc.import` succeeds, and a version field you
can only read by parsing the thing you are versioning is doing less work than
it looks like. Worse, one number forces one answer: any unknown version is
refused, so every additive release becomes a wall.

Two more keys are now reserved and written on every save. Absent — which is
every file written before this build — reads as zero, exactly the promise
those files were written under.

| key | meaning |
|---|---|
| `schema_version` | what the writer used. Informational; gates nothing. |
| `min_reader` | the oldest schema that can **read** this file correctly. |
| `min_writer` | the oldest schema that can **write** it without dropping data. |

Three outcomes, and only three:

- `min_reader` > ours → **refuse whole.** Not opened, not compacted, not
  written. The file is left byte-identical, and there is a test that asserts
  exactly that, because the guarantee is really about what does *not* get
  written.
- `min_writer` > ours → **open read-only.** The writer sees their words; the
  save is refused with "Upgrade Strop to edit it". This also keeps
  `compact_on_open` away, which matters more than it sounds — see below.
- otherwise → **open read-write**, even at a schema newer than ours.

That last line is the point of splitting the numbers. A Loro snapshot
preserves containers a reader has never heard of, so an old build round-trips
a new build's data losslessly — verified for both full and shallow exports.
Protobuf's unknown-field retention, for free, because the CRDT was already
paying for it.

**Why `min_writer` is the sharper of the two, for Strop specifically.** A full
`ExportMode::Snapshot` carries the unread along. `compact_on_open` re-exports
a **shallow** snapshot, and that is the one write path that can quietly
discard what this build never learned to read. Raise `min_writer` whenever an
older build's *save* would lose something; raise `min_reader` only when an
older build would render the document *wrong*.

## 4. Why not a header of our own

The obvious alternative is an outer container — magic bytes, container
version, payload length and hash, reserved space — wrapped around the Loro
blob. It is a good design; it is what SQLite, PNG, Automerge and EBML all do.
It is not worth it here, and the reasons are worth writing down before someone
proposes it again:

- **Identification is already solved.** A `.strop` file already opens
  `6c 6f 72 6f` — Loro's magic — followed by a checksum and a two-byte encode
  mode. `file(1)` and a shared-mime-info entry can key off that today.
- **"Too new" is already distinguishable from "corrupt."** Loro checksums the
  payload and returns a *distinct* error for an unknown encode mode. §2 now
  routes those two errors to two different sentences.
- **The cost is a hard break.** Prepending bytes means every existing file and
  every shipped build disagree about where the payload starts — the exact
  class of failure this document exists to prevent, adopted deliberately.

Revisit if any of those three stops being true — in particular, if the payload
ever needs to stop being a bare Loro snapshot (compression, a sidecar bundle,
multiple documents per file). Then the header earns its break, and the layout
to steal is EBML's: a version for what the writer used, a *separate* one for
what a reader must have, an explicitly-ignorable reserved region, and a
payload length that lets the header grow.

## 5. Sources

The two-number split is EBML's `DocTypeVersion` / `DocTypeReadVersion`
([RFC 8794](https://www.rfc-editor.org/rfc/rfc8794.html)) and SQLite's
read-version / write-version bytes at header offsets 18–19
([file format](https://www.sqlite.org/fileformat2.html),
[compatibility policy](https://www.sqlite.org/formatchng.html)). The
refuse-loudly discipline is Git's `extensions.*` rule, whose stated reason is
the one that applies here — proceeding "risks not only producing wrong
results, but actually losing data"
([repository-version](https://git-scm.com/docs/repository-version)). The
preserve-what-you-cannot-read property is Protobuf's unknown fields
([proto3](https://protobuf.dev/programming-guides/proto3/#unknowns)), which
Automerge's binary format also mandates
([spec](https://automerge.org/automerge-binary-format-spec/)). The
write-gate-because-rewriting-drops-things rule is PNG's safe-to-copy bit
([structure](http://www.libpng.org/pub/png/spec/1.2/PNG-Structure.html)).
Xcode's `objectVersion` is the counter-example: one number, no read/write
split, so every bump is a wall and the gate stopped being obeyed.

## 6. Still open

- **A Flatpak gate in CI.** The manifest was verified by rendering the
  *Welcome* document inside the sandbox — a document that is **born**, not
  opened, which is precisely the path that worked. The gate that would have
  caught this opens a real fixture inside the sandbox and asserts non-zero
  `doc_chars`. It belongs on `flatpak-2026-07`, which has not merged.
- **Two builds, one name.** Both packages report "0.3.0" and the About box
  distinguishes them only by a channel word most writers will never look at.
  Worth deciding whether the packaging channel belongs somewhere more visible
  when two installs collide.
