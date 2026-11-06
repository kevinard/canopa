# Custom paper device precision and driverless acceptance

Status: in progress; scoped maintained-receiver/public-Iced/stock-filter loopback
delivery verified, broader device and physical acceptance remain

The Rust/Iced custom-paper implementation must keep its CUPS approval and document
geometry distinct from downstream driver and physical-device acceptance. This
follow-up remains part of the full portal goal and Print's release gates.

On installed CUPS 2.4.20, the owned custom-range PPD queue resolves 125 × 225 mm
with exact integer dimensions 12500 × 22500 and reported printable margins. The
actual legacy IPP backend sends 12488 × 22471 with 318-unit margins to the PPD
receiver. Three independent CLI jobs using a media keyword, structured media-col,
and both reproduce the same result; adding a collection is not a demonstrated
fix. Canopa's direct output PDF retains the approved page dimensions. Neither
fact establishes exact physical paper selection.

Evidence: `target/portal-print-50/custom-pipeline-01/summary.json`, actual receiver
records in `custom-paper-dark-04/`, and the final phase50 evidence when sealed.
Retain failed attempts and scope limits rather than changing their status.

A driverless fixture attempt failed before upload. The ippeveprinter PPD mode did
not advertise the custom range. Its explicit attribute-file mode produced
duplicate format attributes, and a bounded test relay that retained the first
actually declared format made strict Get-Printer-Attributes pass. Its in-range
media-col then disappeared from retained job attributes despite being present in
the forwarded Create-Job request. The official versioned source is consistent
with that behavior; the exact installed binary's source provenance was not proved.
These experimental fixtures do not establish custom-device acceptance. Their
small sources and raw/forwarded metadata remain archived under phase50; no new
fixture executable, dependency or installed service change was introduced.

Investigate an honest device fixture and a maintained CUPS path that preserve
custom media through delivery. Test at least an interior size, range boundaries,
fractional millimetres, duplex and saved instances. Observe actual submitted and
retained device attributes alongside PDF dimensions, margins and visible content.
Reject silent fallback or distinguish any driver quantization explicitly. Keep
raw input/response evidence and reuse the existing bounded build/native stores.
Do not infer that a mock API response or correct PDF alone proves device media.

No request to patch or install system CUPS is implied. If a maintained companion
or upstream change proves necessary, prepare its concrete source, provenance,
distribution plan and acceptance first. Physical hardware, authenticated devices,
real Flatpak and default Print packaging remain separate open gates.


Phase71 has now reproduced the receiver range-validation fault using only
Validate-Job requests, first on the installed binary and then on the unmodified
pinned upstream C source. A one-condition prototype passes the unchanged ten-case
regression twice. A separate duplicate-MIME capability defect is reproduced and
corrected; the unchanged eleven-case regression then passes twice without a
normalizing relay. No jobs or Canopa requests were created in these probes.
Read [PHASE71-LIVE.md](../PHASE71-LIVE.md) and
`target/portal-print-71/receiver-diagnosis-01.json` before the configured-driver
continuation. The prototype is not a maintained companion or delivery acceptance.

The configured-driver continuation now reaches actual Render/Submit through the
normal worker, a private `-m everywhere` queue, the installed configured filters
and the corrected receiver. A cold 125.01 × 225.02 mm metadata request exposed
two CUPS API pitfalls: lazy catalog creation overwrites the borrowed custom PWG
record, while numeric lookup also normalizes near-standard dimensions despite
exact flags. Catalog initialization followed by explicit-name lookup fixes both,
retaining independent support, exact-dimension and printable-margin checks.
Name lookup alone was an unsuccessful first attempt; its receipts remain failed.

The maintained metadata regression is red on the old binary and green on the
candidate. Four additional cases pass with near-A4 209.99 × 297.01 mm, long-edge
duplex and a saved instance, including both range rejections. The candidate passes
formatting, strict workspace Clippy, the original build graph and 437/55 Portal
library/integration tests. This does not change earlier workspace Files failures.

Actual delivery remains red (pipeline10 and repeated boundary trace11). The
scheduler retains `media=custom_canopa_125.01x225.02mm`, then supplies
`PageSize=125x225mm` to the installed filters. The approved/rendered dimensions
are exact; the receiver request, retained job and PDF are 125 × 225 mm with the
preset's different margins. CLI diagnostic12 supplies explicit custom PageSize:
device attributes and margins become exact, but the received PDF becomes A4.
Diagnostic13 changes the media spelling as well and has the same A4 failure.
These are CLI causal probes, not successful Canopa delivery or adopted fixes.
No host queue, service or physical printer was changed or used.

Next, investigate the scheduler's media-to-PPD choice and the installed PDF
filter's custom-size handling as separate boundaries. Do not merely add PageSize
to Rust submission or loosen geometry assertions: the existing diagnostics
already disprove that complete fix. Preserve configured filters in acceptance.
See [lookup evidence](../print/custom-media-lookup-evidence.json) and
`target/portal-print-71/driverless-pipeline-{10,11,12,13}/summary.json`.

## Phase71 stock-filter correction

The later worker fix orders CUPS option arrays, explicitly marks and validates
the approved PPD PageSize, preserves integer dimensions through float conversion
and derives no further filter scaling. Actual stock-filter delivery passes an
interior fractional size, named control, inclusive minimum/maximum, near-A4,
duplex and a saved instance. The earlier receipts above remain failed evidence.

Prepared PDFs now flatten printed annotation appearances before Rust reconstructs
pages; otherwise stamps were dropped and the installed filter could fall back to
A4. Copy-only PDF output retains original exact physical geometry through a
sealed original descriptor in the isolated qpdf worker. Both annotation loss and
the first candidate's .01pt geometry rounding have actual-worker red/green
regressions. Current470 passes twelve A4/custom cases, three mixed/inherited/odd
duplex groups and native28/29 prepared stamped delivery plus direct30 control.
All received pages have readable labels/nonblank marks and RMS0 filter comparison.
439 library,55 integration and publicPPD1 checks pass; source/runtime pins and
520 retained process identities/private cleanup are verified.

See [ADR0107](../../../docs/adr/0107-exact-paper-and-print-appearances.md),
[the evidence index](../print/paper-appearance-evidence.json) and
[current checkpoint](../PHASE71-LIVE.md). No libppd patch or system change was
adopted. The receiver remains diagnostic: maintain provenance/build recipe before
closing its gate. At that checkpoint, public/Iced revalidation was still pending;
wider device compatibility and actual physical output remain separate. Print
stays opt-in and the full goal active.

The latest public/Iced continuation now passes both dark/100% and light/125%,
including fresh recheck after a width edit and stamped one/two-copy prepared PDF
delivery. Actual receiver attributes, precise PDF boxes, order and stamp pixels
pass with nondefault logical hints. See [public evidence](../print/public-paper-evidence.json).
Initial fixture failures31/32/33 remain: already validated saved defaults, a
bytes/text oracle error, and the documented one-rendered-copy reply respectively.
None required a product change. Maintained receiver provenance is the next gate;
no physical printer, system queue or routing change was used.

## Maintained receiver result

The receiver is now a canonical one-file source overlay with original notices
and a reproducible bounded builder. Its complete source inventory and exact
client configuration/image are verified. A measured direct-execution loader gap
is corrected with a checksum-store RUNPATH; the final program passes all eleven
validation/read-only cases without a library override. The maintained caller is
red on the original unmodified source (seven failures) and green on the correction.
Current public/Iced dark/light delivery also passes on the final receiver.
See [ADR0108](../../../docs/adr/0108-maintained-ipp-receiver.md) and
[the evidence](../print/receiver-maintenance-evidence.json). This closes the
fixture-maintenance follow-up; wider device/physical acceptance remains separate.
