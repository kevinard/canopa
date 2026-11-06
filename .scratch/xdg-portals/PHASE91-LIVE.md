# Phase91: None-veto correction and ordinary Qt FileChooser

Goal ACTIVE; Print deferred. Previous turn made concrete delivery progress, but
its new generator had a real defect and its frontend fixture gave false confidence.
This turn reproduced/fixed that defect and verified an actual ordinary Qt caller.
No Rust/Cargo/vendor runtime source or build changed. All1357guarded runtime
inputs still match accepted phase89 state; normal backend remains dfdf49c5.

## Corrected production defect

`none` anywhere in a portal preference list vetoes the whole interface. The old
generator incorrectly appended it as a fallback after Canopa. Selected disabled
lists now become Canopa alone; inactive names are not revived as fallbacks.
Unselected routes and defaults are preserved. The new unit regression fails
before the fix, then all12unit tests pass. Logs: config-regression-red-01.log and
config-regression-green-01.log; handles54922/59547terminal1/0 respectively.

The previous Settings fixture generated config outside XDG_DESKTOP_PORTAL_DIR's
search directory, so legacy UseIn matching produced its green result. It now
writes to the actual directory and asserts the frontend reports loading that
exact file. This corrected fixture fails with the old generator (no public
Settings interface), then passes public reads/activation/reload with the fix.
Logs: settings-regression-red-01.log and settings-regression-green-01.log;
handle38976terminal1, green invocation terminal0. Do not reuse phase90's result
as proof that the old generator's disabled-list behavior was correct.

## Ordinary Qt result and explicit limit

New scripts/desktop-fixture/file-chooser-qt.cpp uses public QFileDialog/QFile APIs
only, without explicit portal/D-Bus calls. scripts/test-file-chooser-qt.py uses
the generated configuration, real frontend, private Niri and actual Iced helper.
The installed Qt6.12.0 client passes Open/read, Save/write and Cancel, including
filter round trip, Unicode/space paths, original preservation, public+backend
method traces, portal theme mappings and no GTK in caller/helper. Screenshot was
inspected. Native-dark-04/result.json passes3groups, handle77974terminal0.
This uses normal dfdf backend, frozen277568 frontend and installed Niri f09e0b1c
(exact full identities in receipt), not a new companion runtime acceptance claim.

The first three native cases remain FAILED. First24869 exposed the none veto;
second32333 had corrected config but still no portal dialog. Third13591 explicitly
verified public FileChooser v4 and the loaded Qt portal plugin before showing the
same early-constructed dialog. Creating QFileDialog from an event-loop callback
instead then passes all3cases. Qt's async discovery/startup boundary remains a
counterexample; do not call it fixed or universal Qt acceptance. Original caller
sources and cached executable remain available. No Qt fork or Print work added.

Both tiny cached Qt clients total67472bytes; no per-case client/backend copies,
runtime/SDK download or Rust build. Original generator source is losslessly gzipped
and matches its phase90 SHA d804334b. Diagnose workflow used deterministic red/
green tests and source/trace confirmation; no temporary debug instrumentation.
ADR0116 and the chooser/setup guides record the fix and Qt scope.

## Terminal delivery

All current handles terminal. Package4412 and streaming21857terminal0. One existing
development archive replaced, package88.12s/full smoke10.81s. Smoke independently
checks disabled-list correction in the relocated utility and preserves unselected
Print/ScreenCast.41docs/94offline links pass. Streaming proves all8apps+2companions
and both policy files unchanged, corrected7519byte generator and updated guides
shipped, all source pins unchanged; no retained extraction.

Combined: target/portal-filechooser-qt-91/result-01.json.
Archive: target/portal-release-package-check/canopa-x86_64-unknown-linux-gnu.tar.gz
Bytes209602464; SHA256bd3cb68ddd2943e37cad1141c0a2a3fed2e27aba927f9cd4d30864a343aa4739.
Generator SHA256d9a107884a44316cf1c34f18c812baa16f27b4cdb2f662112da38c511ce71811.

Host audit proves37recorded PIDs gone, all4native private paths gone and no Qt/
Settings private environment references or directories. Delivery parent removed;
root remains above3GiB free. Full workspace gate remains86, full Portal library89.
Current tooling results do not relabel old Rust/native matrices as fresh.

This cohort is DONE; do not replay completed gates or package. Continue concrete
non-Print capability/client work from the original map. One actual integration
gap to inspect next is ordinary GTK FileChooserNative: existing real Flatpak GIO
proof calls the public portal explicitly, whereas Qt now exercises toolkit-owned
routing. A GTK test caller is distinct from the Rust/Iced implementation; reuse
installed dependencies and keep the helper GTK-free. Do not turn Qt's startup
limit into another prolonged fork/hardening detour. No external blocker or missing
user answer; Print deferred, full goal ACTIVE, live selection unchanged.
