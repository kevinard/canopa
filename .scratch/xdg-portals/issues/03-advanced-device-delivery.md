# Advanced settings through configured drivers and devices

Status: ready-for-agent

The Rust/Iced controls now validate source, type, quality and resolution with
selected CUPS capabilities and legacy driver constraints. Retain the distinction
between the approved CUPS job, the configured driver conversion and actual device
behavior before enabling Print by default.

Phase51's first public PDF passthrough run retained all four approved settings in
the CUPS job control file. Source/type/resolution were absent from the loopback
receiver's recorded job. Its PPD import also reports a paper-type keyword as a
default source, and the versioned upstream implementation advertises only the
default PPD resolution. The [versioned receiver](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/tools/ippeveprinter.c) is consistent with those import limits. The [CUPS conversion path](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/ppd-cache.c) explains
why a resolution job attribute is not sufficient evidence of raster resolution.
Do not relabel that failed receipt as successful device acceptance.

The corrected scope uses an explicitly declared raster receiver and the existing
installed gstoraster/Ghostscript driver. Accepted Create-Job bytes retain source,
type and quality, correlated to the receiver job ID. Four actual output page
headers contain the selected 600 × 1200 DPI. Both appearances pass the public
Iced flow, including conflict correction and a changed driver before creation.
The receiver's known duplicate MIME attributes are narrowly normalized while
preserving declared values and all job requests/responses. Physical printing and
general driverless/PDF-device acceptance remain open.

Evidence is linked from `print/controls-evidence.json`, including the failed PDF
run, public CUPS control-file inspection, independent CLI encoding probes and
the final raster fixtures. The CLI probe's completion status is not acceptance
of every selected-attribute flag; its earlier page-count expectation was three
while the actual source PDF has four pages. Final native checks obtain that
count from the source and verify all four headers.

Next checks should cover maintained driverless and PDF paths, several supported
resolutions and units, source/type combinations, physical queues and the actual
attributes or raster/header effects at each boundary. Initial application hints and equivalent saved resolution spellings now have
scoped restoration evidence in [ADR0093](../../../docs/adr/0093-initial-print-preferences.md). Preserve prepared-page authority, queue instances and user choices;
do not silently change settings or bypass configured CUPS drivers. Any necessary
runtime/companion changes need concrete source, provenance and scoped acceptance.
Automatic Qt integration remains deferred by the user's choice.

Phase72 continuation: raw receiver capabilities retain four resolutions, including
600 × 1200 DPI and 118 × 236 DPCM, while the stock PDF-only generated queue exposes
only 300 DPI. An explicit configured PPD exposes all three DPI choices. With the
private-client custom-bottom-margin fix, the actual worker can approve and submit
600 × 1200 DPI, but the stock IPP backend omits that attribute downstream. A
test-only forwarding probe preserves it and locates the conversion loss; it is
not a maintained or installed backend fix. Source/type/quality and all four
unequal margins reach the owned receiver. Do not mark this issue complete from
that scoped margin correction or diagnostic result. See ADR0109 and
`print/custom-margin-evidence.json`; multiple-resolution/units, configured backend
compatibility and physical-device acceptance remain open.

Phase73 adds the maintained matching backend and optional relocatable package
(ADR0110). Three DPI choices now survive actual worker/configured-queue/stock-filter
delivery, and standard frontend/Iced consent retains 600 × 1200 DPI in both
appearances. Actual mappings keep the daemon/filter on the system client while
the separately compiled backend uses its matching private client. DPCM passes
the backend ABI regression only; portal/legacy-queue exposure, the stock PDF-only
driverless capability reduction, installed-service selection and physical/raster
acceptance remain open. The earlier interposition probe stays unadopted.

Phase74 source inspection confirms the PDF-only capability loss in the daemon's
PPD generator, with an additional scheduler limitation: PPD resolutions are
advertised as DPI regardless of a choice's suffix. Versioned libppd2.1.1 source
also generates a default and quality presets without a Resolution UI. A private
worker client cannot repair the installed scheduler's generated driver. No
scheduler fork, device-capability substitution or queue-administration feature
was adopted. This remains an explicit compatibility gate, separate from the new
scoped real-Flatpak Print-to-file acceptance; see PHASE74-LIVE.md and the Print
guide's versioned upstream source links.
