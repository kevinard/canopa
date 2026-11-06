# Phase72: declared custom margins; configured PDF resolution diagnosis

Full18 goal ACTIVE, Print opt-in, automatic Qt integration deferred. No question,
approval, agent, commit or PR pending. No installed services, queues, routing or
physical printing changed. Broad check **3178 terminal101**, log
`target/portal-print-72/workspace-01.log`; do not repeat this completed gate. All native
handles terminal; host audit02 confirms205 recorded identities absent and14 private
paths gone. Read ADR0109 and `print/custom-margin-evidence.json`.

Current normal47003b288279.../unit9cce9bee.../integration0444f8ea... unchanged.
NEW private client AD09d247319b6dc5c11d24b86dd011db30027f734e0caf83b3cf63a9b660da40;
recipe a068099810ce5acd5e310867d93f5606d9dfd6b1bf0e5261ef083c6ece7beedf.
New receiver AEF779a4be606c1be24e82a2f7caf8ae73c21e04d910664dd5f65699bed2e69f
matches AD09 via default checksum-relative RUNPATH; eleven no-job checks PASS.
Builds reuse the same target native sources/objects, no configure/Cargo graph.
Client make1.01s; receiver GCC1.3s. Only changed CUPS C source is dest-options.c:
custom range bottom margin was assigned to top fields. New custom-media.patch
changes two assignments to bottom fields, preserving original eight-file auth
patch6ccbc6cc... unchanged. Official1301-file materialization and exact scope pass.
One-time prepare-margins.py cache transition is COMPLETE; DO NOT rerun it.
Receiver cache client-binding transition also complete. New receipts are current.

Metadata06 uniform/09 asymmetric original903 client RED before creation. Same
caller07 uniform/08 asymmetric AD09 PASS, each two sheets accepted/two outside
range rejected, actual worker image mapping checked. Root custom-media checker
now supports --expected-margins LEFT,BOTTOM,RIGHT,TOP in integer hundredths mm.
Asymmetric expected423,529,635,741. Runtime packaging enumerates both source
patches;9 groups incl12 actual worker smoke cases PASS. Current root scripts and
vendor source changes are frozen during workspace validation; later docs only.

Actual configured configured PDF-passthrough delivery12 (AD09+AEF779) preserves12513x18013,
manual/labels/high, long-edge and ALL unequal margins; four PDF pages labels/stamp
and RMS0. No resolution requested in that acceptance run. Driverless fallback13
preserves12513x22513/two copies/eight PDF sides/stamps on1,5/RMS0. Both exact page
boxes within1e-6pt. content-verification-01.json records precise/visible oracles.
BF284 old receiver is losslessly XZ archived/raw retired (79KB), receipt
obsolete-receiver-archive-01.json; AEF779 raw current. Old903 library remains for
baseline evidence. Never inject either private client into a system executable.

Resolution diagnosis, not a production fix:
01 selected600x1200 rejected because generated PDF-only queue supports only300.
02 approved300 retained at scheduler, absent at receiver. Raw receiver exposes
300dpi/600dpi/600x1200dpi/118x236dpcm; raw queue only300. CUPS PDF PPD generator
uses only printer-resolution-default (raster/URF branches differ).
03 test-only backend converter shim forwards300 -> typed wire value PASS.
04/05 configured PPD exposed genuine300/600/600x1200 but margin bug blocked it.
10 AD09 fixes that margin approval/submission; scheduler retains600x1200 but stock
backend still drops it. 11 same caller with target-only shim passes typed wire
600x1200. Source/type/quality arrive via media-col/enum correctly. Marker proves
original converter uses system /usr/lib/libcups.so.2, not private client. Probe is
UNADOPTED and diagnostic only; keep it out of root production sources/package.
A PDF IPP attr alone also does not establish physical/raster resolution.

Additional configured filter14 requires stock pdftopdf and PASS all unequal margins,
exact page boxes/stamps/labels and four pages RMS0; content-verification-02.json.
Native14 terminal0; host audit02 includes it. Poststages40664 terminal0, all4 PASS:
Settings2.12s/CLI2.84s/configuration4.29s/vendor11.88s. Do not rerun them.

Workspace3178 complete101: strictClippy4.54s/testcompile1.84s/Portal439lib+55int
PASS. EXACT same10 Files integration failures as phase71, only failed Cargo target
(596pass/10fail/41ignored). Its private child helper failure is nested evidence,
not another failed Cargo target. Trailing bus shutdown message retained. Full gate
NOT green. Poststages40664 complete0/all4PASS. Results workspace-results-01.json;
final source-guard-02 pins1347 inputs, all Rust/runtime sources and normal/unit/
integration unchanged. Only later ADR metadata differs from guard01.
Evidence34 receipts/20 source-only replays; local links and checksums verified.
No pending tool handle, question, approval, agent, build, native, commit or PR.

NEXT maintain the configured IPP backend resolution correction and verify the
original real caller; do not ship the diagnostic shim. Preserve original
workspace/feature graph and all failures. Current target phase72 ~4MB; ~5.3GiB free;
no per-case ELF, fresh target, release build or cache purge. Root target/debug
preserved. Then maintain a properly built/matching backend correction rather than
shipping the shim; PDF-only capability/multiple resolutions/units remain open.
Physical devices, broader connection06, actualFlatpak/release/deployment/default
Print and six experimental interface gates remain. No real print authorized.

For future native runs supply BOTH --receiver-build target/print-ipp-receiver-compat/maintained/canopa-build.json
and --library target/native-test-snapshots/print-client-by-sha256/ad09d247319b6dc5c11d24b86dd011db30027f734e0caf83b3cf63a9b660da40/libcups.so.2.
The target helper retains historic903 as its default for baseline replay; omitting
--library would retest the old margin bug. Current receiver/worker client choices
are independent processes, never shared private-ABI injection.
