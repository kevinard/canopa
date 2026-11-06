# Exact custom printer paper

Status: scoped CUPS approval, Iced and stock-filter loopback delivery verified;
broader device and physical-printer precision remain open.
Print remains opt-in, outside packaged routing.

Fixed media enumeration cannot represent a custom sheet. The actual CUPS fixture
advertises a custom range and resolves a 125 × 225 mm sheet, while the previous
Canopa catalog exposes only A4. A saved instance with `media=Custom.125x225mm`
also previously fell back to A4. Preserve this observed counterexample rather
than treating a range marker as an approved sheet.

`CustomSize` stores integer hundredths of a millimetre. The Iced fields accept
bounded decimal millimetres with at most two fractional digits; extra precision,
non-finite values, exponents and overflow do not silently round. A custom-capable
printer exposes a separate custom-size control. The user checks a concrete size
before approval. The isolated worker uses CUPS's exact lookup, compares returned
dimensions and retains its printable margins. Duplex lookup uses the duplex flag.
An unavailable size cannot enable approval. A selected saved custom preference
uses CUPS's public `pwgMediaForPPD` parser for the saved `Custom.WxHunit`
spelling, then the same independent concrete lookup. Name lookup alone does not
resolve that PPD spelling. The parser supplies dimensions, never authorization;
the selected queue still validates the exact sheet and printable margins.

The [CUPS public destination API](https://openprinting.github.io/cups/doc/cupspm.html#cupsGetDestMediaByName)
defines integer hundredth-millimetre dimensions and printable margins. Canopa
initializes the media catalog with `cupsGetDestMediaCount` before looking up the
explicit custom keyword with `cupsGetDestMediaByName`, exact flags and an explicit
dimension comparison. A zero catalog count does not reject a range-only device
or authorize a sheet. Two real CUPS counterexamples require this ordering: lazy
catalog construction overwrites a borrowed custom PWG record, and the numeric
lookup normalizes 209.99 × 297.01 mm to A4 even with exact flags. The
[versioned implementation](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/dest-options.c)
checks concrete custom sizes against advertised bounds. These library queries do
not establish physical-device acceptance or every driver-specific constraint.

Custom and named paper are mutually exclusive. The custom media keyword is
derived from the bounded dimensions, rather than accepting arbitrary CUPS option
extensions. An approval carries the concrete geometry. Changes to dimensions or
duplex revoke its confirmation; generation and destination checks reject stale
metadata. The ordinary side, color and copy validation remains in force.

Before Create-Job, the submission worker opens the selected queue/instance again,
resolves the exact sheet with the selected sides and compares all dimensions and
margins with the approval. A changed sheet or printable area fails before job
creation. This is fresh validation, not an atomic promise against a subsequent
scheduler/device change. It does not introduce arbitrary device URI selection.

Prepared applications receive the approved paper setup and render once. Direct
requests use the approved geometry in Canopa's layout worker. Submission does not
reapply page layout to prepared documents. Custom geometry is unrelated to the
editable file-output margins, which remain independently owned by the file draft.

CUPS continues to load only in the isolated worker. There is no new dependency,
GTK backend, service registration, default route or user queue change. The fixture
uses a private scheduler, a custom-capable PDF loopback receiver and existing
checksum-addressed test executables. Preserve its actual CUPS margins rather than
assuming that the fixture's PPD source determines every returned margin directly.

Formatting, strict workspace Clippy and the original default workspace/all-target
compile graph pass. The current binary passes 152 Print tests, 12 Lockdown tests,
the complete 370-test Portal library suite and 55 Portal integration tests;
intentional ignored cases remain. This is not a new complete workspace execution.
Four actual metadata cases verify base and saved mm/cm/lowercase preferences.

Both appearances cover 15 distinct native groups: unsupported or altered paper
before job creation, confirmation and edit invalidation, direct output, saved
preferences, GTK and explicit Qt prepared callers, default selection and a device
capability change between PreparePrint and Print. The light run covers the full
matrix, including scrolling in its shorter 125% viewport. Dark coverage combines
a successful 12-group prefix and a scoped continuation. The prefix receipt's
later Qt blanket-mm assertion remains failed evidence. The corrected check keeps
the actual Qt source PDF and verifies exact source geometry in the received
copies; the toolkit's whole-point rounding is preserved, rather than re-layout.
All 93 observed final fixture processes are terminal. Source and executable pins
match the accepted validation. Phase storage grows about 98 MB, with the 6 GB
reserve intact and no executable copies or new native test programs.

See [the evidence](../../.scratch/xdg-portals/print/custom-paper-evidence.json),
[fixture documentation](../../scripts/desktop-fixture/README.md#custom-printer-paper)
and [checkpoint](../../.scratch/xdg-portals/CONTINUE.md). A
measured legacy CUPS path rounds the downstream device media to 124.88 × 224.71 mm
for a 125 × 225 mm approval, even with structured media options. Exact document
geometry and selected-device API validation do not prove exact physical output.
The [device precision follow-up](../../.scratch/xdg-portals/issues/02-custom-paper-device-precision.md)
retains that limit and the unsuccessful custom IPP/driverless fixture experiments.
Phase71 separately fixes cold fractional-size lookup. The maintained actual-worker
regression fails on the old binary and passes on the candidate; near-A4 dimensions,
duplex, a saved instance and out-of-range rejection are also checked. Formatting,
strict workspace Clippy and the original workspace/all-target build pass, followed
by 437 Portal library tests and 55 integration tests using the compiled harnesses
in their crate working directory. Prior workspace Files failures remain open.
See [the lookup evidence](../../.scratch/xdg-portals/print/custom-media-lookup-evidence.json).

The initial configured driverless delivery test failed. CUPS retained the exact
125.01 × 225.02 mm job keyword but supplied a 125 × 225 mm PageSize to filters;
the received PDF, margins and device request followed that preset. A CLI diagnostic
with explicit custom PageSize preserved exact device attributes but the installed
PDF filter produced A4. Those diagnostic workarounds were not acceptance.
The subsequent worker correction orders CUPS options, explicitly marks and
validates PPD PageSize, preserves integer dimensions through float conversion and
disables additional filter scaling. Actual stock-filter delivery now passes an
interior fractional size, both range limits, near-A4 paper, both duplex edges
and a saved instance, with exact device attributes/margins and visible PDF content.
Prepared PDF normalization also preserves printed stamps and original physical
geometry through one- and two-copy delivery. No libppd patch is adopted. See
[the decision and scoped evidence](0107-exact-paper-and-print-appearances.md).
The receiver remains diagnostic and no physical printer was used.
[Advanced controls and driver constraints](0092-printer-controls-and-driver-constraints.md)
now have separate scoped CUPS/Iced/raster evidence. Authenticated or
physical devices, broader resource/publication/runtime tests, automatic Qt
integration and default Print packaging remain separate work. The full portal
goal remains active.
