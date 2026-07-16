# Repository working rules

*(Read by coding agents — codex/Sol workers and others. Rules here
are operational and mechanically checkable; rationale lives in
docs/. Task-specific scope, anchors, and acceptance come in the
brief, not here.)*

## Toolchain

`cargo`/`rustc`/`rustup` on PATH are snap shims that fail in
sandboxes. Always use the pinned toolchain directly:

```
TC=$HOME/.rustup/toolchains/1.96.0-x86_64-unknown-linux-gnu/bin
RUSTC=$TC/rustc PATH=$TC:$PATH $TC/cargo <args>
```

Setting `RUSTC` is required, or cargo shells out to the snap shim.

## Formatting (HARD RULE)

- This tree is hand-formatted. NEVER run `cargo fmt`, `rustfmt`,
  or any formatter — it rewrites ~1300 untouched lines.
- Match surrounding style by hand: comment wrap ~72-76 columns,
  narrative doc comments that cite spec sections the way the
  neighbouring code does.
- No drive-by reflows, no import reordering, no tidying of lines
  the task does not require.

## Verify — a change is done only when ALL pass

1. The most targeted relevant test.
2. `cargo test --workspace` (via the pinned toolchain above).
3. `$TC/cargo build -p strop-app && bash scripts/rig-check.sh`
   (the headless-sway visual rig; exits non-zero on any failure).

Report every command actually run with its real exit status.
"not_run" and "failed" are legitimate outcomes; never report an
unrun or interrupted test as passed.

Test isolation (HARD RULE — protects the operator's desktop): every
test or app invocation outside `scripts/wrun.sh` runs with the
display env scrubbed and a PRIVATE runtime dir:

```
mkdir -p /tmp/strop-runtime
env -u DISPLAY -u WAYLAND_DISPLAY XDG_RUNTIME_DIR=/tmp/strop-runtime \
  $TC/cargo test --workspace   # (RUSTC/PATH as above)
```

Why both halves: rendezvous sockets are keyed by document path in
the REAL runtime dir — a test that touches a fixed path (Welcome)
can surface the operator's live window; an inherited
WAYLAND_DISPLAY lets a test-spawned binary open real windows on
the operator's screen, which also lets stray keystrokes corrupt
the run. The private dir additionally fixes the `single_instance`
ReadOnlyFilesystem failures under sandboxes. Anything that truly
needs a compositor goes through wrun.sh, which brings its own.

## Scope

- Make the smallest change that fixes the stated behavior; no
  opportunistic cleanup. Route refactors as separate tasks.
- If the fix genuinely needs a file outside the brief's allowed
  list, say so in the report rather than silently expanding.
- Specs are law: `docs/design-principles.md` is the constitution
  (P1-P13); per-feature behavioural specs (e.g.
  `docs/inline-images.md`) govern their features. When a change
  and a spec conflict, stop and report — never silently diverge.
- Unit tests live in-module under `#[cfg(test)]`, in the house
  naming voice (read the neighbours first).

## Never

- Never run `cargo fmt` or any formatter (yes, again).
- Never commit, stage, stash, branch, tag, fetch, or push — the
  coordinator owns all git metadata and authorship.
- Never install dependencies, upgrade crates, or make network
  calls unless the brief explicitly asks.
- Never weaken, delete, or `#[ignore]` an existing test to make
  acceptance pass.
- Never edit files under `docs/` unless the brief explicitly
  scopes them.
