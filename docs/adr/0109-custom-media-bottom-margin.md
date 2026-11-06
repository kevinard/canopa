# Declared custom-paper bottom margins in the private CUPS client

Status: scoped native verified; existing workspace Files failures retained. Print remains opt-in.

The configured PDF-driver continuation exposed a failure before job creation.
For a 125.13 × 180.13 mm custom sheet, CUPS returned the configured left, right
and top margins but retained a 12.7 mm bottom fallback. Canopa's existing PPD
geometry check correctly refused approval. This was independent of the IPP
backend's resolution loss.

The pinned [CUPS 2.4.20 destination parser](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/dest-options.c)
assigns a custom range's `media-bottom-margin` to its top fields. Its later
top-margin assignment hides that first write, leaving the bottom fallback in
place. The additional `custom-media.patch` changes only those two assignments
to the bottom fields. The original eight authentication files remain unchanged.
The patch, official archive hashes and original notices are retained in the
existing root source overlay; no second client fork is introduced.

The regression uses the actual normal worker and an owned private CUPS queue.
Its configured PPD provides the media geometry and constraints. The original
client fails both the uniform-margin and asymmetric-margin cases. The corrected
client accepts the same two concrete sheets and rejects the same two sizes
outside the custom range. The asymmetric fixture independently expects left
4.23, bottom 5.29, right 6.35 and top 7.41 mm. The maintained metadata checker
can assert these four integer values with `--expected-margins`.

Actual submission through a configured PDF passthrough driver and the maintained receiver
also preserves all four unequal approved margins, precise dimensions, duplex,
source, paper type and quality. The four received PDF pages preserve labels and
nonblank calibration marks. This run does not request a resolution. The existing
driverless fallback-margin path is checked separately. A second configured PPD
requires the installed `pdftopdf` filter and preserves the same unequal margins
and four PDF sides. Both filtered runs retain exact page boxes within one
millionth of a point, printed stamps and identical rendered pixels. Canopa's geometry and
pre-creation validation remain strict; no resolver, margin substitution or
direct device submission is added.

The client is rebuilt in the same source/object cache and selected only in
isolated workers. The test receiver is rebuilt against that matching client and
keeps its checksum-relative loader path. Installed CUPS daemons, backends and
filters keep the system library. Runtime packaging retains both complete patches
and checks their integrity alongside the original LICENSE and NOTICE.

Two resolution limitations remain separate. A PDF-only driverless queue generated
by the stock CUPS path exposes only the default 300 DPI despite the receiver
declaring four resolutions, including unequal axes and DPCM. With an explicitly
configured driver, Canopa retains 600 × 1200 DPI in the CUPS job, but the stock
[IPP backend conversion](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/backend/ipp.c)
omits it from the outgoing job. A private diagnostic that forwards only that
option makes the receiver retain its typed resolution, locating the loss. The
diagnostic is not shipped, installed or accepted as a production fix; a PDF job
attribute alone also does not prove physical or raster resolution.

Evidence and source-only replays are indexed in
[the margin and controls ledger](../../.scratch/xdg-portals/print/custom-margin-evidence.json).
Strict workspace Clippy and Portal's 439 library/55 integration tests pass.
The broad run retains the same ten Files integration failures; scoped success
does not establish a green workspace gate. Settings, CLI, configuration and vendor
post-checks pass.
General driverless controls, further resolutions/units, physical devices and
deployment remain open. Automatic Qt integration remains deferred.
