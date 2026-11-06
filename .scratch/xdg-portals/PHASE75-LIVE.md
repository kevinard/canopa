# Phase75: automatic GTK Print inside real Flatpak, scoped acceptance

Full eighteen-interface goal ACTIVE. Print remains opt-in; automatic Qt routing
remains deferred by the user. No installed desktop routing, system/user Flatpak
installation, CUPS service/queue/backend or physical printer changed. No physical
printing. All handles terminal; no pending question, approval, agent, commit or PR.

Read `print/gtk-flatpak-evidence.json` and `print/gtk-flatpak-replay/README.md`.

The new application fixture `scripts/desktop-fixture/print-flatpak-gtk.c` invokes
real GTK3.24.52 `GtkPrintOperation` in `org.canopa.PortalGtkPrintProbe`. It requires
real Flatpak identity, rejects a GTK_USE_PORTAL override, checks that the host-only
sentinel is inaccessible, and exports its actual Wayland parent. Only the Wayland
socket is granted. GTK belongs to this application-side consumer; the unchanged
normal470 backend and actual Iced helper retain their Rust/Iced implementation.
Actual mappings verify the deployed GTK image in the app and no GTK in the helper.

## Final native results

Eight cases pass: dark basic03; dark layout/cancel/withdraw05; light06 all four at
125% scale. The standard frontend277568 forwards authenticated identity and the
exported parent. Basic and layout each return public Prepare/Print response0/0;
cancel returns1 with zero draw calls/no document. Terminating the actual app
withdraws its Iced consent without a Print call or output. All use the real filtered
Flatpak bus and normal opt-in Canopa backend, without broad filesystem/bus/network
grants. Automatic accessibility/Documents-portal setup is disabled.

The controller keeps the GUI app alive until public response and host publication,
then releases its stdin event-loop lifeline. The app hashes its completed private
spool PDF. Each of the four delivered PDFs has exactly those bytes; the backend
does not repeat prepared layout. Basic has four labeled A4 sides and one stamp.
The nondefault case requests pages2–4, reverse sheets, two-up and75% scale, producing
`[4]`, then `[2,3]`. This matches GTK's own draw sequence and existing ADR0076/
Rust Plan semantics. Twelve delivered pages pass explicit system qpdf/Poppler
geometry/text/stamp checks; all six cross-appearance page pairs have RMS0.
Both final appearances and representative readable PDF pages were inspected.

Native host audit: all116 recorded PID/start identities absent across six runs,
including three retained failed setup/oracle runs; all six private directories
removed. No native test, build or fixture process remains running.

## Bounded shared fixture

Reuse `target/portal-print-74/{flatpak-tools,repository,installation,flatpak-build}`;
new receipts are under `target/portal-print-75`. The shared runtime now contains
136 recorded files: the GTK library closure, file print backend, DejaVu font,
Fontconfig configuration, GLib schemas and a443KB evdev/PC105/US XKB subset.
The installed app refs are the GIO and GTK probes plus their shared runtime.
These are owned minimal runtimes, separate from standard-distribution applications.

Checksum-identical raw staging files share the immutable private deployment.
Large newly staged libraries were retired only after archive-payload verification,
then restored as hardlinks after deployment. Only unused CLI translations were
retired after every byte was matched to the retained signed original packages.
The shared fixture remains below128MiB, retaining a3GiB filesystem reserve. No
Cargo build, new graph, release build, cache purge or per-case executable copies.

Current client0afbe090991b4c472bdc86a92d917bbf3b350ad66c36ceb849b2af7a5bc26ce5
is23960bytes, retained once by checksum. Original A402 callback-timing client is
losslessly archived once (5460bytes); its exact original C source is also retained.
The existing legacy GTK client00cdeb and all Rust/native CUPS components remain.

## Retained failures and boundaries

The first SDK export succeeded but its verification used the wrong OSTree root;
the continuation verified `/files/...` without repeating export. First native01
failed GTK startup due missing XKB data, before a Print request. Two direct
XKB-closure probes locate required includes; the final isolated include-path
compile passes. Native02 measured the spool at GTK::done, before the asynchronous
writer completed (zero bytes). The corrected fixture observes after public Print
completion; native03 passes. Native04 delivered exact correct bytes but its oracle
expected logical reversal. Source/ADR/settings/byte evidence confirms physical
sheet reversal; corrected native05 passes. Failed receipts remain failed.

Root changes are the application fixture and two documentation files. Rust/Cargo/
vendor/normal/unit/integration/CUPS client/receiver/backend remain unchanged.
Tooling/native-only validation is appropriate; the workspace gate was not repeated.
Phase73's ten Files failures still prevent full workspace acceptance.

Next: real sandbox original-caller/token ownership and post-preparation lifetime,
using this existing bounded store. Wider standard-app/runtime, Documents export,
printer/device, crash/stalled-filesystem/cross-restart, installed deployment and
release/routing gates, plus the six experimental interfaces, remain open.
Do not repeat completed SDK transitions or native matrices merely on continuation.
