# Exact paper submission and printable PDF appearances

Status: scoped worker, converter, public/Iced and stock-filter delivery checks
pass. Print remains opt-in. The subsequent maintained receiver is recorded in
[ADR0108](0108-maintained-ipp-receiver.md); broader device
compatibility, physical output and deployment remain separate gates.

An approved fractional sheet could become a nearby preset at CUPS's PPD boundary.
An explicit custom millimetre spelling then lost one hundredth of a millimetre
through the driver's float conversion. Correct metadata and a correctly sized
Canopa PDF did not establish correct device attributes. Separately, a printable
PDF stamp disabled the installed filter's no-scaling copy path and exposed its A4
fallback. Canopa's own page-content reconstruction also discarded those stamps.
The failed native receipts remain evidence of these distinct boundaries.

Canopa derives `print-scaling=none` because submitted pages already have their
physical layout. Its CUPS option arrays use case-insensitive name ordering, as
required by CUPS's option lookup. After bulk option marking, public `ppdMarkOption`
selects the exact PageSize explicitly; bulk marking alone can prefer `media`.
An exact matching named sheet remains named. Custom sizes use point strings whose
float representation preserves approved integer hundredths through CUPS's backend
conversion. The worker compares actual marked dimensions and all four margins
against approval, checks conditional limits and conflicts, and retains the exact
PageSize for submission. This adds no caller-controlled option extensions or
driver-conflict resolver. An exhaustive bounded-dimension test checks the numeric
conversion, and the native PPD test uses real public CUPS option lookups.

Recognized prepared PDF printer bodies now use the existing copy-only renderer,
including one copy, alongside PS/SVG. Saved logical layout is not applied again.
Ghostscript receives `-dPrinted -dPreserveAnnots=false`, so printable annotation
appearances enter page content before Rust reconstructs the output. Screen-only
annotations are omitted. This follows the documented
[annotation preservation policy](https://ghostscript.readthedocs.io/en/latest/VectorDevices.html)
and [printed-page switch](https://ghostscript.readthedocs.io/en/latest/Use.html).
Prepared single-copy file output retains its byte-preserving branch.

Ghostscript 10.08's
[page writer](https://github.com/ArtifexSoftware/ghostpdl/blob/gs10.08.0/devices/vector/gdevpdf.c#L1133)
rounds MediaBox coordinates to .01 point, independently of output resolution.
The layout namespace therefore receives an explicit descriptor pin for the sealed
original PDF in copy mode. The existing qpdf parser reads its bounded frozen
bytes inside that namespace. Page counts must agree and converted physical sizes
must remain within .02 point; this check does not relax the output requirement.
Copy sheets and duplex boundary blanks use the original exact physical geometry.
Flattened content retains its converted coordinate system without scaling.
Inherited crop/rotation and UserUnit participate in physical size calculation.
No caller pathname, new parser, Cargo dependency, native executable or GTK backend
is introduced. The private runner still closes every descriptor except stdio
and its explicit file pins on exec.

The maintained actual-worker regression is red before flattening and green after
it for printable, screen-only and Print/NoView stamps, with one and two copies.
Its strengthened geometry assertion is red on the first flattening candidate and
green after retaining original geometry, for both A4 and fractional custom sizes.
Separate native probes cover mixed sizes, inherited crop/rotation/UserUnit and
odd duplex copy boundaries. Production adapter tests verify copy-only requests
despite nondefault approved logical layout.

The actual worker, private driverless CUPS queue, original configured stock filters
and owned receiver preserve exact device dimensions, printable margins and sides
at the interior, both range limits, near A4 and a saved instance. Prepared stamped
PDFs pass one-copy simplex and two-copy long-edge delivery. All received pages
retain readable labels and nonblank marks; before/after filter pixels match, and
stamps remain on the correct copies. These are loopback observations, not physical
printer acceptance. These original receipts use the diagnostic prototype; later
maintained-source/build/loader acceptance is recorded in ADR0108. The exploratory
libppd patch was not adopted or installed.

The standard XDG frontend and normal opt-in backend now pass the prepared stamped
PDF flow through actual Iced consent in dark/100% and light/125%. Editing the
custom width revokes the saved default's current check; explicit rechecking
permits approval. One-copy simplex and two-copy long-edge jobs preserve exact
125.13 × 225.13 mm geometry, device margins, page order and printed stamps.
Nondefault ranges, reverse, scale and n-up remain client rendering hints and do
not re-layout the prepared stream. PreparePrint asks the application for one
rendered copy while the retained approval drives backend copy expansion; the
two-copy body therefore contains eight correctly ordered physical sides.
The backend retains a sealed selected CUPS image without loading it; observed
production workers map that image. All recorded fixture identities are absent
after cleanup. See [public-flow evidence](../../.scratch/xdg-portals/print/public-paper-evidence.json).

Formatting, strict workspace Clippy and the unchanged workspace/default/all-target
incremental build pass. Compiled Portal harnesses pass 439 library tests, 55
integration tests and the explicit native PPD case. Earlier workspace Files
failures remain; no new full-workspace test success is claimed. Source guards,
lossless baseline archives, native receipts and remaining work are indexed in
[the evidence](../../.scratch/xdg-portals/print/paper-appearance-evidence.json) and
[the checkpoint](../../.scratch/xdg-portals/PHASE71-LIVE.md).
