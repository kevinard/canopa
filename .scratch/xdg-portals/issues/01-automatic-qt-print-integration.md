# Investigate maintainable automatic Qt Print integration

Status: ready-for-agent

## Problem and evidence

The installed Qt 6.12.0 Wayland QPrintDialog uses its own dialog and CUPS even
with the xdgdesktopportal theme and a healthy Canopa/standard frontend. The
actual automatic cancel/print probe observes zero Print portal methods. Native
printing completes one exact three-page PDF. A separate explicit Qt D-Bus control
opens Canopa Iced, performs PreparePrint/Print and completes the requested two
copies as one six-page PDF. The automatic gate remains open.

See [evidence](../print/qt-routing-evidence.json),
[decision](../../../docs/adr/0090-automatic-qt-print-routing-gap.md),
[public-API caller fixture](../../../scripts/desktop-fixture/print-qt-native.cpp)
and the exact source references/pins in target/portal-print-49/qt-source.json.

## Investigation deliverable

Find a maintainable Qt client integration path that sends ordinary QPrintDialog/
QPrinter applications through the standard Print frontend and Canopa Iced.
Inspect available public/plugin hooks, current upstream work and narrowly scoped
companion options. Record ABI/version, licensing, ownership, distribution and
resource requirements before choosing an implementation. Read vendor-maintenance
instructions before modifying or materializing an upstream fork. Retain the
existing automatic probe as a real caller; the application itself uses public Qt
printing APIs and has no hand-written portal requests.

## Acceptance for a future implementation

- Ordinary QPrintDialog presents Canopa Iced and the private bus records its
  automatic PreparePrint/Print calls from the actual caller.
- Cancel creates no print job; approval delivers the exact three pages once with
  the requested copies/layout and current printer options.
- Original Qt parent/caller lifetime, request cancellation, worker retirement and
  uncertain acknowledgements preserve the existing portal ownership contract.
- The separate explicit D-Bus control remains distinguishable in receipts.
- Scope includes the selected Qt version/companion and a reviewable distribution
  path. Real Flatpak and other Qt versions require their own evidence.
- Validation reuses root Cargo outputs and checksum-addressed native fixtures,
  respects the 6 GB reserve, and avoids a full Qt SDK rebuild unless justified
  by a concrete resource plan.

## Comments

The user chose to finish Canopa Print features first. Qt client integration
remains tracked for later; the full portal goal and automatic Qt acceptance stay
open. This is a priority decision, not approval for a particular SDK fork or
deployment. Independent Canopa backend work can proceed.
