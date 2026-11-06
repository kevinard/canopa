# Phase73 sealed: matching optional IPP backend and runtime-pairing guard

Full18 goal ACTIVE, Print opt-in, automatic Qt integration deferred. ALL handles
terminal; no pending question, approval, agent, build, native, commit or PR.
No installed CUPS service/backend directory/queue/device URI/desktop route changed,
no physical printing. Read ADR0110 and print/ipp-backend-evidence.json.

CURRENT backend D3b6fd7ce7ab9fd8c62b2054132934883319f0fd7b64b7f91733bbf3417e3b98,
113512bytes, shared print-backend-by-sha256 store. Client AD09 and receiver AEF779
unchanged. Normal47003b288279... / unit9cce9bee... / integration0444f8ea... unchanged.
Canonical backend recipe47a2bb3c84d910cf58139cdcb7979e66a725a43ed1090d493ad10151f2f07fd8,
patch336a213b78c5015ccb21de6d2320fc63a9f12cfabc2f0a21e794c283ad080d9e.
Only backend/ipp.c changes from pinned official2.4.20: preserve explicit typed
printer-resolution after PPD mapping; require Canopa capabilityv1 before private
APIs. Strong imported symbol refuses stock-client fallback before discovery.
No interposition/default synthesis/driver bypass. Old phase72 shim UNADOPTED.

Builder verifies both1301file inventories, exact private headers/config3files/
client image, compiles5Cunits once, publishes one113KBprogram. Stable sourcecache
at target/print-ipp-backend-compat/maintained, reserve3GB/growth256MB/time120s.
Only matching compiled backend uses private APIs; never inject client into stock
executables. Actual daemon/filter retain systemCUPS E70, workers use sealed AD09.
Default backend loader selects matching sibling in package or checksum store.
No new Cargo graph/fresh target/release build/cache purge/per-caseELF copies.

Maintained root scripts: build-print-ipp-backend.py, test-print-ipp-backend.py,
package-print-backend.py, test-print-backend-package.py; advanced-ipp.conf fixture.
ABI regression five cases:300/600/600x1200dpi/118x236dpcm/absent. DPCM is ABI-only,
not portal/legacy-queue exposure. Package final ABI02 TENgroupsPASS incl relocated
actual sibling loader, stock-client missing-runtime refusal and five owned IPP
jobs. Source/config/image mismatch refusal3PASS before creating buildcache.
Bash/YAML/four negative release-argument checks and packaged docs36files/75linksPASS.
Release opt --cups-ipp-backend requires --cups-runtime; smoke --require-print-backend;
workflow explicitly builds/stages/checks both, configures no CUPS service.

Original compiled backend1238 ABI600 and actualconfiguredqueue04 RED (resolution
absent). Initial corrected4C79 ABI5PASS, configured05..07PASS; relocated2479
configured09..11/public12dark13light PASS, but discovery-only probe showed stock
fallback when matching client unavailable. NO deviceURI/job in that probe.
Final D3b6 hardens startup; source/cache transition prepare-abi-guard.py COMPLETE,
DO NOT rerun. Auth clientAD09 / its private headers / receiverAEF779 unchanged.
Official materialization02 proves full1301inventory and exact one-file patch.

FINAL normalworker/configuredCUPS/stockpdftopdf/matchingbackend/receiver matrix03
cases14..16 PASS three DPI choices, tray1/manual, stationery/labels/glossy, quality
3/4/5, simplex/bothduplexedges, unequal margins423/529/635/741, fractional12513x18013,
four pages each/labels/stamps/RMS0. Actual image mappings prove backendAD09 while
daemon/filter E70. Final PUBLIC17dark100/20light125 PASS standardfrontend277568 +
normal470 + actualIced + D3b6 package + stockfilter, each1copySimplex/2copyLongEdge.
Approved initial tray1/labels/high/600x1200 and custom12513x22513 explicitrecheck
retained to typeddeviceattrs. Four/eight exactboxes/labels/stamps; logicalranges/
reverse/scale75/Nup4 not reapplied. Mainnotmapped/retainedsealed15 image; actual
workers mapmatchinginode. Both appearances' confirmed captures inspected.

Failed final-light18 delivered BOTH correctjobs but sampling missed short-lived
filter;19 deliveredboth plus exactloadertrace but oracle rejected CUPS-escaped tabs.
Both receipts remain FAILED fixture evidence. Corrected target-only parser tested
against retained19 logs (filter-loader-parser-01), then20returns0, per-filterPID
ELF initialization logs prove E70. No library override, no filter replacement;
only loader diagnostics around original stock filter. Backend/scheduler sampled.
Two early setup errors are separately retained, not product failures.

Hostaudit02:23reports/654 recorded identities absent/26 private paths removed.
Baseline1238/intermediate4C79/preguard2479 raw executables losslesslyXZarchived;
D3b6 rawcurrent. Final small component pair is losslessly archived at
 target/portal-print-73/canopa-print-runtime-and-backend-guarded.tar.xz
248476bytes (~243KiB); native-archives-final-02 verifies every member checksum.
Raw hardened-prefix removed after finalnative/audit. Restore archive's canopa-print
as hardened-prefix for old staged-path cases; for future tests use D3b6 checksum
binary and matching AD09. Earlier unguarded component archive is historical ONLY.
Never rerun one-time build-baseline.py or source/cache transitions as setup.

FINAL workspace55373 terminal101 AFTER guard: strictClippy4.46s/testcompile1.89s,
Portal439lib/55integration PASS. EXACTsame10 Filesintegration failures as phase72,
onlyfailedCargotarget (596pass/10fail/41ignored), fullworkspace NOTgreen. Preguard
34456 also terminal101/sameten preserved separately, not final acceptance.
Finalpost88349 terminal0/all4PASS Settings1.39s/CLI1.73s/config2.66s/vendor9.32s.
Earlierpost69304 terminal0 too, historical. DO NOT repeat completed checks merely
because of continuation. Guardfinal02 pins1355rootinputs/currentbinaries; no Rust
source change, only later ADRstatusmetadata differs from frozen ABIguard01.
Evidence53receipts/18source-onlyreplays before final documentation seal.

NEXT advance stock PDF-only driverless capability reduction/multiple-unit portal
coverage while preserving configuredqueue authority. Backend ABI DPCM is not that
proof. Explicit installed-service selection, physical/raster device behavior,
broaderconnection06/actualFlatpak/release/deployment/defaultPrint and remaining
sixexperimental interface gates stay open. Host previously lacks FlatpakCLI/
libostree/libflatpak; no real printing authorised. Current free space~5.2GiB;
root target/debug preserved, no newRustgraph or active handle to wait/restart.
