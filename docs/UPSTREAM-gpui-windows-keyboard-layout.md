# Keyboard-layout independence for shortcuts (and an upstream gpui-on-Windows gap)

Status: Linux works today; Windows is broken by code analysis (not yet
confirmed on hardware); macOS is uncertain and shares the risk. **The Windows
fix is now shipped as `patches/gpui_windows-keyboard-layout.patch`, applied via
a zed fork** (see `patches/README.md`) — the route chosen after cargo-patch
proved unworkable for a git-sourced dep on ephemeral CI. This file remains the
analysis + the on-hardware verification ask.

## The principle

A keyboard shortcut names a **physical key**, not a glyph. By the principle of
least surprise, `Ctrl+Shift+P` must open the command palette no matter the
active layout: a bilingual writer mid-Russian-sentence presses the same
physical key (which now types `з`) and expects the same command. The same
holds for `Ctrl+C`/`Ctrl+V` and every other chord. The character the key
*would* type is irrelevant once a non-shift modifier is held.

This is how Strop's chords are written (`commands.rs`, `bind_keys`): `ctrl-c`,
`ctrl-shift-p`, … — Latin key names. The question is whether each platform
resolves a physical key press back to that Latin name when a non-Latin layout
is active.

## Per-platform status

### Linux (Wayland + X11) — works

`gpui_linux` resolves the keystroke key from the xkb keysym, and when the
produced character is **not** a single ASCII char (e.g. Cyrillic `з`) it falls
back to a hardcoded evdev-keycode → US-QWERTY table:

`crates/gpui_linux/src/linux/platform.rs`
- `guess_ascii(keycode, shift)` — keycode 33 (physical `P`) → `'p'`, etc.
- the keystroke builder takes that branch precisely when `!key.is_ascii()`.

So physical `P` under the Russian layout yields keystroke key `"p"`, and
`Ctrl+Shift+P` opens the palette. Verified by reading the pinned gpui rev; no
Strop change needed.

### Windows — broken (by analysis)

`gpui_windows` builds the keystroke key from the virtual-key code via
`MapVirtualKeyW(vkey, MAPVK_VK_TO_CHAR)`, which translates using the **current
layout**:

`crates/gpui_windows/src/keyboard.rs`
- `get_key_from_vkey` → `MapVirtualKeyW(.., MAPVK_VK_TO_CHAR)` → for `VK_P`
  under Russian this returns Cyrillic `З`, so the keystroke key becomes `"з"`.
- there is no "modifier held → fall back to the US Latin letter" path.
- the layout-remap table that *does* exist (`CANDIDATE_VKEYS` /
  `get_vkey_from_key_with_us_layout`) covers only OEM punctuation and digits —
  **letters are deliberately excluded**, and it runs at binding-load time, not
  on input.

Windows assigns `VK_A..VK_Z` by **physical position** regardless of layout
(this is why most Win32 apps' `Ctrl+C` survives a Russian layout — they read
the VK code, not the character). gpui instead re-derives a character from that
VK through the active layout, which re-introduces the layout dependence the VK
code had already removed. Net effect: under a Cyrillic layout, letter-key
chords (`Ctrl+Shift+P`, `Ctrl+C`, `Ctrl+B`, …) would not match.

It can't be fixed from Strop's own code: the keystroke is produced inside
`gpui_windows` before the keymap sees it, and there's no app-level hook to
rewrite it. So the fix lives in the gpui tree, carried as
`patches/gpui_windows-keyboard-layout.patch` and consumed via a zed fork
(`patches/README.md`). The patch is three lines — return the US-layout letter
directly for `VK_A..=VK_Z` (whose VK codes are the ASCII letters) before the
layout-sensitive `MapVirtualKeyW` call — so a chord stays Latin-named whatever
the active layout. An upstream PR remains the eventual home; the fork just
unblocks Strop's Windows users in the meantime.

Recommended upstream fix: in `get_key_from_vkey` (or its caller), when the
keystroke carries `ctrl`/`alt`/`win` and the layout-mapped character is
non-ASCII, fall back to the US-layout letter for `VK_A..VK_Z` /
`VK_0..VK_9` — mirroring what `gpui_linux::guess_ascii` already does. File
against zed-industries/zed, `gpui_windows`.

### macOS — uncertain, same class

`gpui_macos` uses `charactersIgnoringModifiers` plus a key-equivalent map
(`get_key_equivalents` returns `Some` only on macOS). That handles non-US
**Latin** layouts well, but Cyrillic-only setups are a known macOS hard case.
Untested here. Should be verified with the same checks as Windows once a macOS
build exists.

## Verification ask (Windows testers)

With a non-Latin layout active (e.g. Russian) **and no Latin layout selected**:

1. Does `Ctrl+Shift+P` (physical `P`) open the command palette?
2. Does `Ctrl+C` / `Ctrl+V` copy / paste?
3. Do `Ctrl+B` / `Ctrl+I` toggle bold / italic in the document?

If all work, this analysis is wrong (Windows is delivering Latin VK characters
for letters) and we can close this. If they fail, the upstream fix above is the
path. Either way, record the result here.

## Note on Strop's `bind_keys`

`bind_keys` loads registry chords with `gpui::DummyKeyboardMapper` and
`use_key_equivalents = false`, i.e. it stores chords verbatim and applies no
platform layout remapping at bind time. For the Cyrillic letter case this is
irrelevant (the candidate table excludes letters either way), but switching to
the real `cx.keyboard_mapper()` would additionally fix punctuation chords on
non-US **Latin** layouts on Windows/macOS. Left unchanged for now — it touches
the working Linux path and is unverifiable locally; revisit alongside the
upstream letter-key fix.
