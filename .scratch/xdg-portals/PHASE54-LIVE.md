# Phase54: Iced printer authentication

Previous goal turn: progress. Phase53 worker boundary is sealed, but complete
authenticated printing and the full18 goal remain active. Eleven packaged,
six experimental, Print opt-in. Print-first/Qt-deferred user priority persists.
No pending input or external blocker.

Current implementation under validation: request-local Broker with serialized
Iced prompts, endpoint/protection/realm/resource or queue/ordered-fields cache,
rejected-cache invalidation and username preservation, six interactive prompts
versus 64 bounded worker credential exchanges. Clearing for file output rejects
late answers. Cache/provided answers are cleared/redacted Secret owners; UI
messages wrap edited values privately. Dedicated helper returns only a bool on
ordinary output and a bounded binary reply over a private socket. Metadata and
submission use the same provider. Prepared approval transfers a bounded sealed
anonymous memfd over the existing private descriptor channel; Grant/settings stay
credential-free. Approval retains the request cache until use/expiry. Normal file
handoff is unchanged. Child helpers receive the original appearance arguments.
Interactive operations have a finite300s budget; each prompt90s, worker channel120s.
Caller/operation cancellation closes prompts; cleanup remains unattended.

Lint01 failed an unused import, fixed after terminal. Lint02/45909 PASSED
186.06s with unchanged guards. Inspection then found that completion moved/cleared
form values before a possible last view. The view now guards completion, with
post-completion view regressions. Message Debug also redacts keyboard events.
Candidate01 is frozen:22files/261605bytes, manifest and copies under
 target/portal-print-54/candidate-01-sources.json and sources/candidate-01.

Fullvalidation03/52552 is LIVE: original workspace/default/alltarget/offline/
incremental/jobs1, root cache; reruns lint for these final edits and then all
Portal/Print/Lockdown/integration/explicitPPD cases. No guarded source edits while
live. Poll the same handle to terminal, not a fresh invocation on timeout.
Baseline6887645184free,6GBreserve/3GBphasecap. No dependency/vendor/routing change,
binary copies/new native executables/fresh trees/cache cleanup/installed services.

Native source /tmp/canopa-print-54/native.py is parsed but NOT RUN. Pass explicit
CANOPA_PRINT_VALIDATION=.../worker-03.json only after it passes; dark then light
(CANOPA_PRINT_TEST_SCHEME=light), sequentially. Reuses pinned normal frontend/Niri/
pointer/parent/installed wtype (credential input on stdin). A temporary private
scheduler and chunked Basic relay preserve job requests and compare literal
queue fields in memory; only known-owned printer URI metadata is rewritten.
Expected eleven public groups per appearance: wrong/retry/Enter metadata,
prepared cache handoff and literal queue values, direct, actual GTK/explicit Qt
prepared callers with six exact pages, sign-in/device Cancel beforeCreate, Close, caller loss,
Lockdown and shutdown. Synthetic secrets, scheduler spool/auth files and trust
state are temporary and removed after process retirement. No Iced authentication
acceptance yet. Inspect actual stable masked-form images before acceptance.

Full authentication still requires protected recovery, late Create/Send account
versus durable owner binding, held-job correction without document resubmission,
Digest, longer credentials, and broader real GTK/explicit Qt/native/physical
acceptance. These remain required, not redefined away by UI boundary acceptance.
Broader Print failure/resource/driverless/Flatpak/runtime/default packaging, six
experimental companions and finalworkspace/release/tenFiles gates remain.

Full03 strict workspace Clippy PASSED152.06s; build still live on52552 per polling.

Validation03 build passed751.55s; Print184/12ignored and Lockdown12 passed.
The same52552 handle continues Portal library/integration/PPD. No Rust changes.

Future investigation ideas, NOT verified claims: test actual scheduler Basic/PAM
principal binding in a disposable mount/user namespace, with private passwd/PAM
files and synthetic credentials, rather than the current Basic gateway's Unix
forwarding identity. The gateway does not establish real scheduler principal
resolution. Check available bwrap/PAM/runtime capabilities first; no host auth
files or credentials may be changed. For held device jobs, investigate an owned
401 device endpoint and CUPS backend authentication-required state, then update
the same accepted job using CUPS-Authenticate-Job. Existing accepted journal
records are removed; retaining accepted authenticated jobs until terminal/held
outcome will need an explicit ownership/state design. No document resubmission.

Full03/52552 TERMINAL0:184Print/12ignored,12Lockdown,402Portal library/20ignored,
55integration/2ignored,explicitPPD1; guard unchanged, netgrowth307720192bytes.
Native auth-worker01/95943 PASSED14 and controls-worker01/95450 PASSED27/34processes.
Native authentication-dark01/72785 got externalSIGTERM143 after2successfulgroups;
39recorded processes audited/retired usingPID/starttime/pidfd, bothownedtmp roots
removedafterlivenessaudit. Failedreceiptkept. Causeunestablished (parallelsource
fixtures were active); do not claim parallelcausation. Native02/74968 terminal
FAILED at explicitQt deviceprompt timeout; fourprefixgroups pass: metadatawrong/
retry/Enter, preparedhandoff/literalcredentials, direct, actualGTK sixpages.
All92observedprocesses terminal, temporaryschedulerremoved byfinally. Qtlogempty,
not a confirmedproductcause. DiagnosticQt-only source/tmp54/qt.py nowrunning;
follow nexttoolhandle. It startsQt first, preservespassedRust03 andallguards;
no Rust changes/rebuilds. Live nativehandle recorded in tooltranscript. Other
six native cancellation/lifetime groups and all light/ordinaryUIregressions still
pending. No Icedfullacceptance/seal yet. Promptformdark actualmasked/retry view
visually inspected. Hypotheses showedbeforeprobe: conversiondelay, priorresource
retention, Qt/requestresponse boundary. No productiondebuginstrumentation.

Qt-only01/44509 failedsamebeforedeviceprompt: PDF notcreated; extraCUPSGetDefault
request afterPrepare. Qt-only02/6784 PASSED1/33terminal afterchangingONLY theQt
client CUPS_SERVER to nonexistentprivateUnixsocket; backendstillusesprotectedreal
ownedrelay/scheduler. PDF7727bytes exists and6expectedpages preserved. QPrinter's
native scheduler lookup blocked its PDF renderer. This is explicitQt portal
isolation, NOT automaticQt/authrouting acceptance. Fullnative.py uses thissame
client-only env; originalfailedfixtures immutable. Fullauthdark03nowlive (tool
handle), noRustchanges orrebuild. No debugchanges inproduct.

Full authentication-dark03/28150 PASSED all11groups;132observedprocesses terminal,
secret-output comparisons pass. ActualGTK and explicitQt sixexpectedPDFpages,
Cancel metadata/no reprompt, deviceCancel beforeCreate/nojob, publicClose, caller
loss, liveLockdown and normalbackendshutdown allpass. Allownedtemporaryscheduler/
auth/spool data retired. Lightauth01 nowlive (toolhandle). OrdinaryIceddark/light
6groups each stillpending. NoRustedits sincevalidatedcandidate01. No fullfeature/
protectedrecovery/heldjob/longcredential/Digest/defaultPrint acceptance claim.

Lightauth01/92558 FAILED externalSIGTERM15 atwindowmove beforedeviceCancelcase;
seven successfulprefixgroups, all117recordedprocesses terminal andtempdata removed.
Externalcauseunestablished, not productfailure orparallelcausation. ParentGDB
light02/54247 then nativePASS11/131terminal without receivingSIGTERM. GDBexit1 is
only finalcontinue afterPythonnormalexit; p$_siginfo=void. No credentialmemory
inspected, onlysignalmetadata; nohostsecuritychanges/newexe/debugdownloads
(debuginfodprompt answeredN). ChildCanopa/GTK/Qt/Niri processesdetachednormally.
Bothappearances realmaskedserver/queue/retryforms visuallyinspected. Lightfull
acceptanceuses thiscontroller-debuggerrun explicitly, not theinterruptedprefix.
Ordinarydark01 nowlive (nexttoolhandle); ordinarylightpending. Noacceptanceseal
orfullAuthdoneclaim; allguardedRustedits stillvalidatedcandidate01unchanged.

Ordinarydark01/5588 PASSED6groups/29terminal. Ordinarylight01 nowLIVE nexttoolhandle.
/tmp54/seal.py prepared/ASTchecked, NOTRUN; requiresall6finalnativereceiptspassed,
75groups,terminal/pins/sourceguards/secretprotocol source,actualGTKQt6pages,
4rasterpages600x1200DPI,debuginfoddisabled/nativeundercontrollerdebugger caveat,
failedreceiptsunchanged, startupdependency/layout/doclinks/whitespace/storagegates.
Runonlyafterordinarylightpassed andactualordinarycapturesinspected. Finalgoal and
fullauthremainopen. Allnativeauthhandles terminal; onlyordinarylightactive.

Sourceinspectionfound additionalrequired issue05: PartialJob.drop->Journal.retire/
cancel opensfreshSessions whileworkerScope.current isstilllive. OnlyouterClient
cleanup passesNone. No newnativefaultreproyet, but neverclaim allautomaticcleanup
unattended. Issue05 recordsrequiredprompt-suppression/privatefaulttest before
completeauth/defaultPrint. NoRustmutationsnow; scopedUIpositive/cancelacceptance
remainsasmeasured. Sealprose/limits corrected; nextgoalturnmustaddressissue05.

## Phase54: Iced authentication scoped verified; protected recovery next

Full18 goal ACTIVE: eleven packaged interfaces, six experimental and Print opt-in.
User priority remains Canopa Print features; automatic Qt integration deferred.
Previous goal turn was progress. No live validation/native/debugger handles,
pending input or external blocker. Do not mark the complete goal or complete
printer authentication done.

Implemented one shared Iced authentication helper for metadata and submission,
explicit masked input and endpoint/transport/realm or exact queue/field-order
consent. Credentials use private binary sockets; the ordinary helper reply is a
bool. Request-local cache invalidates rejected credentials, preserves the edited
username, and separates six prompts from64 bounded cached exchanges. File
selection clears the cache and rejects late answers. Prepared printing transfers
a bounded sealed anonymous memfd over the private approval socket. Grant/settings,
argv, ordinary output and recovery records contain no passwords. Existing token
owner/expiry rules retain the private approval. Outer supervisor cleanup has no
provider; synchronous partial-job retirement still needs explicit suppression of
interactive prompts and protected-fault acceptance (issue05).
Interactive operation/prompt/channel bounds are300/90/120s. Completion view and
message-debug regressions cover cleared values and keyboard text redaction.

Validation03 passed:184Print/12ignored,12Lockdown,402Portal library/20ignored,
55integration/2ignored and explicitPPD1. Formatting, strict workspace Clippy152.06s
and original workspace/default/alltarget/offline/incremental/jobs1 build751.55s
passed. No full workspace test execution; ten existing Files failures remain.
Candidate01:22files/261605bytes. Binary SHA256:
b6a6c7b4c9722f8f416110cefeb0f001f8f3a1d1712e65a78e4887e8cf2de68c.

Final native75groups/370observedterminal processes: worker auth14, controls27,
Iced auth11 perappearance, ordinary public Iced6 perappearance. Wrong/retry/Enter,
prepared cache after helper exit, literal device values, direct, actualGTK and
explicitQt six-page prepared output, sign-in/device Cancel beforeCreate, Close,
caller loss, Lockdown and shutdown pass. Both masked forms/appearances inspected.
Ordinary choices/conflicts/driver changes and four600x1200DPI raster pages remain
correct. These use private CUPS and synthetic credentials; scheduler spools/auth
files are temporary and removed after retirement. No physical or real PAM
principal binding is established by the Basic gateway.

Failures retained: unused import lint01; authdark01 externalSIGTERM143 (two prefix
groups,39recorded processes separately retired and two temp roots removed after
PID/start-time/pidfd audit); dark02 and Qt-only01 blocked before PDF rendering.
A Qt-only differential proves QPrinter's native scheduler lookup blocks that PDF
fixture. Only the Qt client's CUPS_SERVER is now a nonexistent private socket;
Canopa still uses the protected real scheduler. ExplicitQt passes; automatic Qt
routing/authentication stays deferred. Light01 externalSIGTERM15 after seven
prefix groups remains failed. Full light02 passes with GDB on Python controller
only; children detached, no SIGTERM reproduced, signalinfo void. GDBexit1 is only
continue after normal exit, not a native test failure. No security setting or
production debug code changed; no debug downloads. External signal cause unknown.

Authoritative: target/portal-print-54/acceptance-01.json and
print/iced-authentication-evidence.json. Same root cache/stores; no dependencies,
vendor/routing/installed queue/service changes, binary copies, new native
executables, fresh build trees or cache cleanup.6GBreserve/3GBphase growth cap;
final observed storage in the sealed receipt.

NEXT: suppress interactive prompts in synchronous partial-job retirement (issue05),
then actual protected scheduler account binding, authentication first required
atCreate/Send, durable owner recorded before uncertain creation; interactive
protected recovery and identity-checked CUPS-Authenticate-Job for the existing
held job, without document resubmission. Digest, longer credentials, timeout/
stalled-helper and native file-output credential-clearing checks remain required.
The6-prompt cache is for an individual print request; full recovery-batch policy
still needs integration and acceptance. Inspect PHASE54-LIVE's explicitly
unverified private-PAM/held-backend investigation ideas before choosing a seam.

Broader Print device/driverless/custom precision/physical/Flatpak/failure/global
resource/runtime/default packaging, six experimental companions and final
workspace/native/package/release/tenFiles gates remain. No commit/PR or goal
completion, pause or block.
