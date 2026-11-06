# Phase53: printer authentication

Previous goal turn: progress (application preference implementation and scoped
validation sealed in phase52). Full18 objective active;11 packaged/six
experimental/Print opt-in. Finish Print first, Qt automatic integration deferred.
No live processes at phase start, no pending input or external blocker.

Plan full authenticated-printer behavior: server HTTP auth and ordered per-job
auth-info; Rust-only request-scoped secret channel; explicit Iced consent; bind
credentials to selected server/queue and current requirements; cancellation,
wrong credentials and changed endpoint/requirements tests. No secrets in normal
worker stdout, command line, public settings or recovery records. No keyring or
on-disk credential persistence. Private test credentials only.

Start with primary source and actual CUPS API probes before choosing callback
contracts. Reuse rootworkspace/default/alltarget/offline/incremental/jobs1 and
checksum native stores.6GBreserve/3GBphasecap, baseline in target/portal-print-53.
No binary copies/new native executables/new build tree/cache cleanup/installed
queues or routing changes. Phase52 receipts immutable.

Primary versioned sources retained upstream.json (five sources total~360KB).
API01/90245 terminalFAILED at an overstrict single-callback decline expectation:
real cupsGetDests2 invoked callback for both listing/default requests. Good
credentials changed cupsUser and listedonequeue. Decline status4096/noauthorized
posts; allownedprocesses terminal. Rust must latch refusal so a second underlying
CUPS request cannot reprompt. API02 next measures remainingcases.

API02/53577 PASSED10cases: Basic189bytes user:password succeeds,190+truncates;
explicit decline invokes underlyingcallbacktwice; wrongthenDecline threecalls.
Escaped multipleauthfields preserve values; oneescapedfield doesNOT; literal
IPPAddStrings does. Nojobs/usersecrets. API03/78839 FAILED firstcopyDestInfo
query; allownedprocesses terminal. Diagnosingfixture/API beforemanualIPPsubmit.
An officialsourcefetch auto-review timed out (not an unsafe-action finding);
browserprimarysource fallback selected, no userpermissionquestion.

Unvalidated Rust inprogress: print/authentication module typedmetadata/secret
binarychannel/cancellation/refusal; workerInput optionalauthenticationFD and
explicit AuthCancelled event; Sessionstablecallback neverconsole; currentpeer
transport +TLSreconnect requirement; orderedmetadataRequirement; literalIPP
CreateJob helper preflightscredentials beforejournal. Client stillsendsNone: Iced
provider/retainedapprovalcredentialhandoff remains REQUIRED fullfeaturework, not
complete. Newmodules maystillhavecompile/lint/nativeissues; noRustcheckrunyet.

API04 failed same metadata URI issue because CUPS omits :631; API05/58645 PASSED
10cases with knownowned printerURI mapped for implicit/explicitport. Actual
versions[1.0,1.1,2.0,2.1] andURI reflecttheproxyendpoint; noTLSupgradeclaim was
inferred. No jobs/usercredentials.

Lint01/7623 failed PollFd.new borrowing; fixed. Lint02/67182 PASSED strict whole
workspace/alltarget Clippy165.05s. Candidate01 snapshot14files/169826bytes
nowfrozen, fullvalidationworker03 live (handle follows commentary/tool record).
Reusepassedformat/lint receipts withidenticalguards; sameoriginalgraph no-runbuild
andscopedPrint/Lockdown/allPortal/integration/explicitPPDtests. Escalated run avoids
phase52's sandbox socket problem. NoRust/Cargo/vendor edits whilelive.

Worker03/73645 TERMINAL0: strictClippy02reused165.05s, samegraphbuild682.23s;
173Print/12ignored,12Lockdown,391Portallibrary/20ignored,55integration/2ignored,
explicitPPDruntime1pass. Sourceguardsunchanged; nofullworkspaceexecution.
Net storage measured -11735040bytes atvalidationend. Nativeauthworker next;
source-onlycontrols/public ordinarydark/light regressions adapted in/tmp53.

Nativeauth01/45374 and02/8656 FAILED positive listing beforeanychallenge;
unattendedcasepasses, all3observedprocesseseachterminal. Nonsecretlast_operation
recordsreject2/noauthorized/challenges[]. Straceprobe03/80481 failed unavailable
tool(allownedterminal). GDBattach04/85845 refused byYamaptrace_scope1; nosecurity
settingchanged/allownedterminal. Diagnosefeedbackloop andrankedhypotheses used.
API06/59763 PASSED10, confirms socket_fd=-1 duringCUPS callback (sameHTTPobject);
connectionvalidation hypothesis confirmed. API07 next tests public reconnect in
callback beforeanycredentials; no Rustmutationyet, candidate01/worker03 still
currentvalidatedsources but nativeauthNOTaccepted.

API07/31285 PASSED10: publichttpReconnect2 inpasswordcallback returns0, opensfd5
aftercallbackfd=-1; actualAuth+metadata succeeds. Officialhttp.c explicitly
preservesheaderfields duringreconnect/TLSupgrade. Rootcausefix applied AFTERall
priorhandlesretired: reconnectsameHTTPobject beforepeercheck, forcealreadyTLS
policy atsessioninstall, preserveALWAYS. No debuglogginginproduct. Candidate02
archived; lint04/22030 live orterminalpertool handle. Fullrebuild/nativeoriginal
repro/ordinaryregressions stillrequired. Privateworker.pydebug branchesremoved;
failednative01/02 anddiagnostic03/04 preserved.

Lint04/22030 PASSED113.04s, guardsunchanged. Fullworker05 nowlive (nexttool handle),
reuses04format/lint withsameinputs, originalgraphsamecache. Candidate02
14files/170995bytes. Nativeworkerloopmustuseworker05 onlyafteritpasses. No
guardedRusteditswhilelive. Iced/fullauth/heldjob/longcredentialgates remainopen.

## Phase53: authentication worker verified; Iced integration next

The full 18-interface goal remains active: eleven interfaces are packaged, six
are experimental, and Print is opt-in. The user chose to finish Canopa Print
features first; automatic Qt integration remains deferred. No required input or
external blocker is pending. All validation and native processes are terminal.

The worker now has a private inherited credential stream, bounded typed
challenges and binary replies, redacted/cleared secret owners, refusal latching,
and cancellation. CUPS callbacks never fall back to console input. They check the
actual Unix/loopback/TLS peer, preserve TLS policy, and reconnect the same public
HTTP object when CUPS closes its socket before invoking the password callback.
Ordered queue requirements use literal IPP text fields, preserving punctuation
and single-field passwords. Queue credential preparation precedes the journal.

Corrected validation worker05 passed: 173 Print tests (12 ignored), 12 Lockdown,
391 Portal library (20 ignored), 55 integration (2 ignored), and the explicit
PPD runtime regression. Formatting, strict workspace Clippy and the original
workspace/default/all-target/offline/incremental/jobs1 compilation passed.
No complete workspace test execution was established; ten existing Files failures
remain. Sources are frozen in candidate02: 14 files / 170995 bytes. Binary SHA256:
e68dd7d275c0205e419339f14c3dd51e0c68206e80aa2816fcd166819e618462.

Final native acceptance covers 53 groups / 109 observed terminal processes:
14 actual authentication-worker cases, 27 ordinary worker cases, and six public
Iced regression groups in each appearance. Basic/TLS, exact UTF-8/punctuation,
refusal, wrong credentials, malformed replies, caller-pipe loss, ordered queue
fields and refusal before Create are checked. Ordinary Iced controls and four
600x1200 DPI raster pages remain correct; dark/light captures were inspected.
These tests provide no Iced password-prompt or protected recovery acceptance.

The original callback failure and API differential remain evidence. CUPS calls
its password callback with fd=-1; public reconnect repairs the peer check. Later
native05/06 failures were a fixture relay dropping chunked document bodies.
Transport-size observations confirmed an empty forwarded body; native07 passes
after forwarding chunks intact. Production code did not change for that fixture
failure. Failed receipts retain their status. No debugger/security setting was
changed and no production debug logging was added.

NEXT: integrate Iced authentication for metadata and submission, request-owned
reuse and a private retained handoff for prepared printing. Reconcile finite
interactive deadlines and distinguish user prompts from cached recovery-batch
exchanges. Verify late Create/Send authentication, authenticated account versus
durable job owner, protected recovery, and identity-checked held-job correction
without resubmission. HTTP Digest and credentials beyond the current 189-byte
Basic boundary remain required. Client.task still sends authentication=None.
Do not mark authenticated printing complete. See issues/04 and ADR0094.

Authoritative receipt: target/portal-print-53/acceptance-01.json; evidence:
print/authentication-evidence.json. Reused root cache and checksum stores; no
binary copies, native executables, target trees, cache cleanup, dependency/vendor,
installed queue/service or routing changes. A 6 GB reserve and 3 GB phase growth
cap were retained; final storage is recorded in the receipt.

Broader device/driverless/physical/Flatpak, publication-crash/stalled-kernel and
cross-restart resources, Print runtime/default packaging, six experimental
interfaces/companions and workspace/native/release gates remain. No commit or PR
and no goal completion, pause or block.
