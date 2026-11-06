# Printer controls and legacy driver constraints

Status: scoped CUPS, Iced and raster-delivery verification complete.
Print remains opt-in and unadvertised.

The owned advanced PDF queue advertises source, paper type, quality and resolution
through CUPS's destination API. That API does not report the fixture's
Manual/Glossy constraint and accepts a 125 × 225 mm custom sheet beyond the
manual tray's 127 × 203.2 mm limit. Independent support checks cannot authorize
the complete selection. These are measured CUPS 2.4.20 results, retained under
`target/portal-print-51/advanced-query-01/` and `ppd-query-01/`.

Canopa exposes four fixed controls, using advertised IPP values and bounded
labels. Quality remains the standard enum; resolution retains both dimensions
and its DPI or DPCM units. There is no arbitrary CUPS option editor. Saved IPP
values and the corresponding InputSlot, MediaType, cupsPrintQuality and Resolution
driver aliases are resolved explicitly. Unavailable, conflicting spellings and
ambiguous aliases fail metadata discovery rather than silently selecting a
different preset. Individually available defaults that conflict together remain
editable in the dialog.

The isolated worker checks public IPP support and combinations, then reads the
selected scheduler's PPD when present. The compatibility adapter uses the
[public PPD API](https://openprinting.github.io/cups/doc/api-ppd.html), which CUPS
has deprecated. It is required only for a queue that actually returns a PPD.
An HTTP 404 follows the IPP-only path; other download or parse errors fail the
check. The PPD is bounded to 1 MiB, streamed through a pipe and retained in a
sealed memfd. It creates no temporary disk file and executes no driver code.
The library remains loaded only in the worker.

The library handle is retained for the worker process's lifetime. An explicit
runtime test reproduced a CUPS thread-exit callback jumping into an unloaded
library after its session ended. The debugger located the thread-exit crash;
retaining the same library with a preload made the unchanged test pass. A lazy
process-wide handle fixes the ownership boundary without adding a startup link.
The original failed receipt and debugger/A-B observations remain retained.

Each mapping or validation starts with a fresh opaque PPD object. The public
choice, option and size records are bounded before use. IPP source/type keywords
are mapped through CUPS's media mapping; standalone source/type fields were
observed to leave the PPD marks unchanged. Mapping from different starting
choices detects ignored aliases. Submission carries the validated fixed driver
choice alongside its canonical IPP value. Driver defaults and installable options
are marked before the explicit draft. Conflicts and conditional custom-size
limits are checked without invoking a resolver.

The Iced check is bound to its destination, generation and complete device option
set. A stale or changed response cannot authorize a later draft. Standard device
edits trigger another check; custom size changes require explicit confirmation.
Copies are checked as one transport copy because Canopa materializes physical
document copies separately. A fresh check repeats before the intent journal or
Create-Job, using the currently selected driver. This is not an atomic promise
against subsequent scheduler or device changes.

Prepared clients receive bounded source/type, quality and integer-DPI rendering
hints using GTK-compatible setting names. The installed GTK scalar API was
measured for those names and asymmetric axes. DPCM is rounded upward only for
the client's integer-DPI rasterization hints; the selected printer option keeps
its original units and dimensions. These hints do not reformat an application's
already prepared pages. Initial application preference restoration is described in
[ADR0093](0093-initial-print-preferences.md), separately from a selected queue's defaults.

Formatting, strict workspace Clippy and the unchanged workspace/default/all-target
compile graph pass. The scoped suites pass 155 Print tests, 12 Lockdown tests,
373 Portal library tests and 55 integration tests, with intentional ignored cases.
The separately invoked public PPD runtime regression also passes without a
preload. The earlier crash and failed checks retain their original status.

Eighteen real worker cases cover defaults, IPP/PPD aliases, unavailable values,
combination conflicts, conditional custom limits, raw-queue metadata and fresh
pre-Create rejection. Five reconnect cases pass after the library ownership fix.
Dark and 125% light appearances each pass four public advanced-control groups,
including a real dropdown correction and a driver change before Print. Source,
type and quality are verified in the accepted Create-Job request; all four raster
pages carry the selected 600 × 1200 DPI. Existing installed CUPS/Ghostscript
programs perform the driver conversion. Each appearance also passes six custom
paper regressions, including actual GTK and explicit Qt prepared clients.
All 139 observed final fixture processes are terminal. No new native fixture
executable, binary copy, dependency, build tree or installed routing was added.

The first PDF passthrough run failed its downstream-settings assertion. The CUPS
job control file retains all four approved values; the downstream path omits
resolution, while the PPD-import receiver also has problematic media defaults.
A separate raster fixture verifies resolution in the actual page headers. Its
attribute-file receiver declares its capabilities explicitly. A narrow relay
removes only duplicate MIME attributes generated by ippeveprinter, preserving
the first declared values and retaining raw/forwarded metadata. Requests and job
responses are unchanged. This establishes scoped protocol/driver delivery rather
than physical output or all legacy PDF drivers. See
[the downstream follow-up](../../.scratch/xdg-portals/issues/03-advanced-device-delivery.md).

See [the evidence](../../.scratch/xdg-portals/print/controls-evidence.json) and
[fixture notes](../../scripts/desktop-fixture/README.md#advanced-printer-controls).
This work does not resolve
[downstream custom-paper precision](../../.scratch/xdg-portals/issues/02-custom-paper-device-precision.md),
authenticated devices, physical printing, automatic Qt routing, the broader
failure/resource gates or default Print packaging. The full portal goal remains
active.
