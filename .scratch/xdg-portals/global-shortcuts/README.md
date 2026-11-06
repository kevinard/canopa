# GlobalShortcuts implementation and evidence

Status: experimental Rust/Iced v2 backend, unadvertised, 2026-11-01. Owned public
sessions, once-only Bind, List/history and Iced Configure now pass 34 shortcut
checks; all Portal checks pass 213 library/55 integration, formatting and strict
Clippy. Eighteen public native groups pass in dark/100% and light/125%, with all
ten Iced helpers terminal and no inherited backend/compositor sockets. See
[session evidence](session-evidence.json) and ADR0067. The newer focus-scoped
inhibition implementation now passes 22 final editor groups across both appearances,
three native retained-handle/window-lifetime rounds and six shared desktop suites,
including concurrent editors and retained reservations. See
[inhibition evidence](inhibition-evidence.json) and ADR0068. Keyboard recording,
broader acceptance and distribution remain open.

The revised companion also passes 48 existing native lease/input regression
groups, with all 16 owned processes terminal and no guarded input changes. Three
real Wayland/XKB preparation cases establish modifier ordering, remapped Caps Lock
and AltGr rejection requirements; see [recording preparation](RECORDING.md).
Full-workspace strict Clippy and all-target compilation pass. The original gate
finishes with the same ten Files failures; all other Rust and post stages pass.
There are no live validation jobs. `acceptance-01.json` in the phase27 artifacts
reconciles all source/binary guards and parses the exact main failure lists.

The [canonical private compositor integration](COMPOSITOR.md) retains 53 core
and 50 scoped native groups on recipe a750dae3… / compositor 9d8ccdd3….
Its required workspace gate completed with the same ten Files failures; all
other Rust and post-Rust stages pass. The earlier [transport checkpoint](transport-evidence.json)
retains fifteen tests and three native groups, including backend SIGKILL with a
held trigger. The original investigation and baseline history follow.

Original investigation: The
[contract/source investigation](../GLOBAL-SHORTCUTS.md) identifies an existing
dynamic accessibility-grab API and the proposed compositor lease/session seam.

The predicted release-order gap is now reproduced in two private native runs on
the existing frozen Niri (`9b9ba92c…`). With the explicit US-layout fixture,
Ctrl+a delivers press and release when a is released first. Releasing Ctrl before
a delivers only the press. An independently grabbed F24 press/release confirms
processing after the tested sequence. The second run also observes for half a
second after that marker. Each synthetic keyboard stays alive until observations
are captured, separating a missed release from source disappearance. All owned
processes retire, with unchanged input hashes.

[evidence.json](evidence.json) links the exact receipts and checksum-addressed
probe snapshots. This verifies the existing accessibility seam's gap; it is not
GlobalShortcuts backend or physical-input acceptance. Those baseline probes did
not rebuild the compositor or application. Later prerequisite work is recorded
below; the live session is unchanged.

Next, implement keyboard recording,
then broaden geometry, layout, lifetime, sandbox and distribution acceptance. Preserve
accessibility ABI and ordinary input behavior. Print remains unimplemented;
eleven interfaces are packaged and six are experimental.

[Lease implementation preparation](LEASE-PLAN.md) retains the historical staged
core and the independently reproduced source-disconnect correction. Its staged
51-test files have been superseded by the canonical 53-test compositor modules;
use the current recipe and compositor ledger for further implementation.
