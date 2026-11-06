## Phase56: durable authenticated Create owner scoped verified

Full18 goal ACTIVE:11 packaged/six experimental/Print opt-in. User chose Canopa
Print features first; automatic Qt integration remains deferred. No required input
or external blocker. No live builds/native fixtures remain. All toolhandles from
this phase, including16012/95607/6612, are terminal; never restart old checks merely
because a tool session is unavailable. Full goal/full authentication are NOT done.

Corrected late Create login ownership. Only Create arms a journal binding. Before
CUPS can retry with an approved username, callback persists that provisional owner;
failed writes refuse authentication. Guard disarms beforeID/Send so existing owner
stays fixed. No schema, dependency, driver, runtime profile, route or installed
printer/service change. Four Rust files/70252bytes, source candidate01, complete
300-line review patch with preimages verified against phase55 hashes. No source
changes since that snapshot. Binary SHA256:
d0273396ab98dbcc7b162a51480af69b526eb41529f259ac58f769b6ea54f440.

The original actual-PAM lost-Create regression remains RED in principal-probe01.
Corrected actual private PAM/passwd overlays use a synthetic alias/password,
never host auth files/real credentials. Four variants/12cases verify ownerdurable
and unknownID BEFORE Create forwarding, exact cleanup afterlostreply, retained
correct owner on workerdeath, no authenticatedCreate after refused journal write,
and unchanged original owner for Send-time login. All28observed fixture processes
terminal and private PAM/password/spool data removed. Send-only401 remains a known
failure, explicitly excluded from feature completion; no document retry added.

Scoped evidence: print/creation-owner-evidence.json and
`target/portal-print-56/acceptance-01.json`. Sixteen fixture receipts total103 native
cases/503 observed terminal processes. Includes12actualPAM,7retirement,14authworker,
27controls,5reconnect,4publicIcedfailures,11authentication and6ordinaryprint groups
perappearance. Bothdark100/light125, actualGTK/explicitQt prepared callers, wrong/
retry/Cancel/Close/callerloss/Lockdown/shutdown, driver/raster regressions pass.
Actual masked auth and ordinary controls visually inspected in bothappearances.
No debugger/externalSIGTERM failure in this phase. Historical failed evidence stays.

Validation01 PASS: format3.00s,strictworkspaceClippy189.06s, original workspace/
default/alltarget/offline/incremental/jobs1 graph653.24s.192Print/12ignored,
12Lockdown,410Portal library/20ignored,55integration/2ignored,PPDruntime1 pass.
Source guards unchanged. Same rootcache and checksum native stores; no new native
binary, binary copy or dependency. Netphasegrowth37023744bytes, final6717132800free,
6GBreserve/3GBphasecap retained. Phase55 had gained184041472bytes off its baseline.
No complete workspace execution; ten existing Files integration failures remain.

ADR0096 scopedverified; ADR0095 phase55 retirement scopedverified(91cases/480terminal),
its source/binary evidence preserved. README/devguide/issue04/issue06 updated with
current limits. Source-only replay wrappers, validator, sequence and guarded sealer
are preserved in `target/portal-print-56/replay/`; current /tmp56 copies also exist.
No commit/PR or goal status change. Issue06 remains in-progress for Send and protected
account/owner authority aftercreation. Do not claim full authentication or defaultPrint.

NEXT: actual Send-only authentication and protected recovery. Source confirmed
cupsFinishDestDocument/cupsGetResponse performs authentication/reconnect after401
but never replays stream. Direct authenticated rawIPP Send succeeds; worker sends
one unauthenticatedSend, receives401, approves callback, thenfails and keepsincoming
job/record. This is definitive rejection, distinct from lostfinal acceptance.
No general retry; continuation needs finite attempts, sameexisting job/tag/queue/
immutableowner/endpoint, fresh identity and checkedconnection. Current Server.user
still combines recordedowner/expectedaccount; Create correction does NOT solve
this aftercreation. my_jobs=1 filtersbycurrentauthaccount, so blind reuse of recovery
lookup for differentactor/owner is insufficient. Unattended startup also refuses
aliasowner before UI if configuredOSuser differs; explicit protected review still
needs an authorized account path. Preserve cache-only automatic cleanup, accepted
jobs, caller/Lockdown/Close withdrawal and reconnect cancellation guard.

Suggested next probes: raw actualPAM GetJobAttributes/numberofdocuments afterSend401,
owner vs authactor visibility/refusal, then bounded continuation only afterexplicit
rejection with identity revalidation. TLS handshake may alter final HTTPstatus;
httpReconnect2 preserves401 on successful plain reconnect, not a complete TLS proof.
Capture real protocol facts before a transport redesign. Helddeviceauthentication
occurs afterscheduleracceptance and needs CUPS-Authenticate-Job on samejob, neverSend
again. Digest/longcredentials, native timeout/stalledhelper/fileoutputcache clearing,
physical/driverless/Flatpak/deviceprecision, crash/stalledFS/resources/runtime/default
Printpackaging remain. Sixexperimental companions and workspace/native/release/
tenFiles gates remain. Do not spawn subagents unless an applicable explicit rule asks.
