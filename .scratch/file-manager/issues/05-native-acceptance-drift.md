# Native Files acceptance: fixture drift after the 2026-11 interface rework

Status: done (two cases blocked on packages; coverage gaps listed below)

## Context

On 2026-11-14 the complete native Files matrix (168 `verify-files*` cases in
`scripts/desktop-fixture/acceptance.json`) ran in private Niri 26.04 sessions
against one debug build of the working tree (ADR0174–0177 on top of `0470785d`).
**50 passed and 118 failed.** Each failing case was then rerun against a build of
committed `0470785d` alone, and **all 118 fail there too**. No case passed on that
commit and failed on the working tree, so the failures predate these changes.
`files-iced-transfers-dark` is flaky on both builds (it failed and passed on each).

Receipts, per-case errors for both builds and binary checksums are in
`.scratch/artifacts/files-acceptance-2026-11-14/` (`manifest.json`,
`batch-a`, `batch-b`, `head-failed`). Python Pillow came from a private
virtualenv; the machine has no system Pillow.

## Causes

- **Coordinate drift.** Most fixtures click fixed logical coordinates from
  before the toolbar, menu and places rework (`20ed53b6`, `d947bb62`). The
  Search button moved from the second toolbar row (311, 105) into the location
  field (208, 64), and menu rows moved. Examples: `files-iced-search` (fixed,
  below), `files-devices` (the device row click opens Trash), `files-undo*`
  ("Timed out waiting for Rename/New folder/Completed": menu items not hit),
  `files-iced-clipboard` ("Copy through the real context menu").
- **Missing tools on this machine.** `files-dnd-cancel-gtk-sway` needs `sway`,
  `files-clipboard-protocol` needs `wayland-info`, and `files-dnd-transport` and
  `files-dnd-cancel-control` need the `drag_transport` example built first.
- **Flow changes.** Several cases wait for states or titles the reworked
  interface no longer produces in the same order, such as Quick Look titles in
  `files-reload` and `files-hidden-navigation`, and painted-selection probes in
  `files-downloads`. These need a look at each case's `failure.png`.

## Done

- `files-iced-search-{dark,light}` pass again: the Search click targets the
  magnifier, and the case now also checks the ADR0175 **Search contents**
  option natively (the content-only token is found and opened when it is on,
  and is not actionable when it is off).

## Remaining

Update the fixtures case by case against `failure.png` and the current layout,
preferably replacing fixed coordinates with positions read from the AccessKit
tree (`scripts/native_accessibility.py`), so later layout changes do not break
them again. Install `sway` and `wayland-info` on the acceptance machine, or record
those cases as environment-gated.

## Comments

- 2026-11-14 (agent A, `verify-files.py` cases): 59 GTK-reference cases were
  removed. They drove the removed GTK frontend's GApplication actions,
  mnemonics, titles and notification actions, and never ran against Iced
  (ADR0018). Behaviour they covered that has no Iced case yet is listed as gaps
  in the report.
- The remaining 87 cases were repaired; 83 pass on the working tree
  (`.scratch/artifacts/files-acceptance-repair/A/final*`).
- Open: `files-iced-interrupted-*` and `files-iced-source-leases-*` fail on
  operations/store bugs: a refused reconcile rewrites the journal, and released
  retained bytes are not cleaned up.
- Open: an intermittent vendored winit "configure for dead window" panic.
- Fixed in the app: a narrow Quick Look header, Seek focus lost during slow
  seeks, and shortcut-row Record recording its own Return.
- Details: `.scratch/artifacts/files-acceptance-repair/A-report.md`.
- 2026-11-14 (agent B, other drivers): 19 of 21 cases pass. The drift: Properties
  is a fixed-size floating window, devices moved to the Places menu, Rename now
  selects only the name, the device helper's message format is now version 3, and
  notification titles are capitalised. `files-dnd-*` need the `drag_transport`
  example next to the `--files` binary and the Niri fork build. Report:
  `.scratch/artifacts/files-acceptance-repair/B-report.md`.
- 2026-11-14 (parent, closing):
  - `files-iced-interrupted-*`: the fixture was stale, not the journal.
    ADR0147 replaced automatic return with refused reconciliation and Keep in
    Recovery. Once the taken copy has left its name, retrying completes the Undo
    and keeps the work folder for Recovery. The fixture now checks exactly that:
    the name is untouched and the bytes preserved, Keep retires the work, and the
    salvage keeps its own Undo. The app now reports "still holds preserved work …
    keep its work folder in Recovery" instead of the raw "Directory not empty
    (os error 39)".
  - `files-iced-source-leases-*`: the refused Rename is a Failed request, which
    keeps its own reservation until **Dismiss request** (ADR0105). The fixture
    now dismisses it, so the release step tests the clipboard lease alone.
    New-folder chords lost under load are resent only while neither the prompt
    nor the folder exists.
  - `files-iced-search-*`: More+Stop is retried when the larger scan finishes
    before Stop is read, so the stopped-scan check is still exercised.
- **Final matrix** (`.scratch/artifacts/files-acceptance-final/`): one frozen debug
  build of the working tree (`binaries.sha256`). All 107 stock-Niri cases pass:
  89 in the first two parallel batches, the rest when rerun serially on a quiet
  machine (`serial`, `quiet`, `quiet2`). Every failure in the loaded runs was an
  IPC, screenshot or compile timeout, or lost injected input; another session was
  building Niri meanwhile, at a load average of about 8. The three drag-and-drop
  cases pass on the fork build (`fork/`).
- Still open:
  - `files-dnd-cancel-gtk-sway` needs `extra/sway`, and
    `files-clipboard-protocol` needs `extra/wayland-utils`.
  - The pinned winit fork can panic with "got configure for dead window" (it
    needs a fork PR).
  - The removed GTK-era cases left gaps with no Iced native case yet; see the
    table in A-report.md.
