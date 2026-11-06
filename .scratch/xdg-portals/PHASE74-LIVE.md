# Phase74: real Flatpak Print file caller, scoped acceptance

Full eighteen-interface goal ACTIVE. Print remains opt-in; automatic Qt routing
is deferred by the user. No installed portal routing, CUPS queue/service/backend,
or physical printer changed. No physical printing. All native/build handles are
terminal; no pending question, approval, agent, commit or PR.

Read `print/flatpak-evidence.json` and `print/flatpak-replay/README.md`.

## Completed

The new application-side `scripts/desktop-fixture/print-flatpak-client.c` uses GIO,
with no GTK dependency. One persistent public portal connection runs prepared
Print, direct Print or cancellation. A real Flatpak deployment provides
`/.flatpak-info`; the app requires `org.canopa.PortalPrintProbe`, cannot access an
existing host-only sentinel, and sends an unlinked sandbox PDF descriptor.

The standard frontend277568 and unchanged normal470/Iced backend pass all three
flows in dark/100% (`native-dark-03`) and light/125% (`native-light-04`). Recorded
implementation calls carry the authenticated Flatpak ID. Both successful output
paths contain four ordered labeled A4 pages and the printable first-page stamp.
Cancellation returns1 and sends no document. The isolated app has no host
filesystem, network or CUPS-socket grant. Automatic accessibility/Documents-portal
setup is disabled; this is scoped Print FD/file acceptance.

`flatpak-pdf-content-01.json` checks all16 output pages using explicitly pinned
system Poppler/qpdf executables. Prepared output matches the original input;
direct output matches a separately executed actual normal-worker layout control.
Every comparison has pixel RMS0 and exact A4 boxes; stamp ink256 on page1 and0 on
the other pages. Both appearance captures and readable page renders were inspected.

Native host audit:102 recorded PID/start-time identities absent and all four
private desktop directories removed. The first failed fixture retired services
after directory teardown, leaving only two Mesa cache files; ExitStack ordering
was corrected before subsequent runs and those exact files were safely removed.
Failed initial audit and explicit cleanup receipts remain.

## Environment and retained failures

Flatpak1.18.4, OSTree2026.4 and three small dependencies were checksum/signature
verified from official Arch packages, then extracted only under
`target/portal-print-74/flatpak-tools`. No pacman install or host/user Flatpak
installation was changed. A workspace-only installation contains the owned app
and a9.6MB runtime library closure. Total fixture storage is about45MB, capped
at128MB, with a3GB free-space reserve in its private repositories. No Cargo build,
fresh graph, cache cleanup or copied per-case executable. The27KB client lives
once in the shared checksum store (F8a75e247a700079454dfda75d181af16b9b7fbce6e6969eac58b8f4a52ce0f2).

Retained failed setup receipts: runtime files-directory selection, default OSTree
3% reserve, missing build-finish, parentless tiled window, and cross-filesystem
PDF rename after successful handoff. Final native runs use floating fixture
windows and copy only the small PDF. Do not relabel the failed receipts as green.

The first raw-source pixel oracle wrongly required direct layout to match an
unnormalized PDF. A separate normal-worker control establishes the intended
Ghostscript result. A sandbox control failed converter execution; unchanged native
host control passes. Bundled Codex Poppler selected through PATH rendered missing
glyphs even when invoked on the host. Independent system Ghostscript and explicit
`/usr/bin/pdftoppm` render readable labels. Retain those failed visual receipts;
future native PDF checks must select the actual system renderer explicitly.

## Driverless boundary and next work

Source inspection confirms CUPS2.4.20 PDF-only generated PPDs expose the default
resolution; the scheduler interprets PPD choices as DPI. Versioned libppd2.1.1
generates a default/quality presets without a Resolution UI. A worker-only client
change cannot repair a stock daemon's generated PPD. No scheduler fork, direct
device capability substitution or queue-administration feature was adopted.
Issue03's driverless/DPCM portal exposure and physical/raster gates remain open.

Normal470, unit9cce, integration0444, clientAD09, receiverAEF779 and guarded backend
D3b6 remain unchanged. Root changes are the C fixture and two documentation files.
This tooling/native-only phase does not justify repeating the shared workspace
gate. Phase73's ten Files integration failures still prevent full workspace green.

Next: extend actual Flatpak acceptance to a real toolkit caller using the existing
bounded installation/library store, then printer/identity/lifetime cases where
applicable. Standard application runtimes, Documents exports, printer/device and
installed deployment, crash/stalled-filesystem/cross-restart gates, six experimental
interfaces and final routing/release acceptance remain open. Reuse completed proofs;
do not rerun one-time deployment continuations or replace this scope with a claim
of full Flatpak/Print completion.
