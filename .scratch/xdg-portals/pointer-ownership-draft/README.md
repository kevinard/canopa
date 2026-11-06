# Draft compositor pointer ownership foundation

Status: unadopted, 2026-10-31. These patches extend the exact canonical
`niri-remote-input` base recorded in `draft.json`. They are review artifacts;
`vendor/provenance.json`, the installed compositor, portal main registration and
shipped capability descriptor remain unchanged. Niri retains its GPL-3.0-or-later
license and Smithay its MIT license from the existing pinned recipe.

`niri.patch` and `smithay.patch` change seven files, including one added Rust
module. Device identity uses actual typed equality, with collision-safe lookup
and identities that are not reused. The first source starts a combined button
cycle; its last owner ends it. Removing a source cancels its pending release
binding even after its own release, while preserving another source's held state.

Cleanup distinguishes seat-routed and consumed presses, retains bookkeeping for
consumed compositor gestures, cancels affected explicit grabs before clearing a
whole retired batch, and keeps tablet suppression separate. Host focus loss
retires the host pointer alone. A full input reset clears all pointer cycles and
preserves the existing global-reset release-action policy. Ordinary source loss
never runs release bindings or confirms a drag/drop.

Nine model tests pass, including all 576 four-device press/retire orders. The
seven native suites pass 38 checks on one frozen binary; the main evidence ledger
records its hash and timings. The seven-file delta replays with exact hashes
against the preserved canonical base. This does not register a new canonical
recipe or replace full archive/workspace/distribution acceptance.

`native-proof/` preserves the private fixture sources. They use isolated desktop
services and harmless receivers. The host cases use synthetic Xvfb input, and do
not establish physical hardware acceptance. Drag setup waits for a same-client
Wayland sync callback before injector motion; an unfenced diagnostic and delayed
client reproducer are retained alongside the corrected control.

See [the evidence and continuation ledger](../POINTER-OWNERSHIP.md) for failures,
exact artifacts, limitations and the remaining pointer lease/mapping work.
