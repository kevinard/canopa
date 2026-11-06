# Preserve accepted printer jobs after bookkeeping failure

Status: scoped native and Iced validation complete; full Print remains opt-in.

A private scheduler accepted one document and returned complete HTTP200/IPP0.
Immediately before forwarding that final response, a disposable hard link made
the local receipt catalog fail its security checks. The worker then emitted
Created/Failed with "Invalid file destination" and cancelled its received job.
The job ended at state7 with one document. This reproduces with unmodified native
CUPS and cups-exec; it is separate from a lost or rejected scheduler response.
See the [measured investigation](../../.scratch/xdg-portals/print/held-job-investigation.json).

The complete scheduler acknowledgement is the submission commit point. Commit
in memory before any later receipt write. A failed acknowledgement write leaves
a warning and retained metadata, but the worker still reports Accepted and cannot
enter partial-job cancellation. Successful bookkeeping retains the existing
accepted flag and metadata retirement behavior. Credentials and document bytes
never enter an additional journal.

Bookkeeping failure can leave an older durable record that does not know about
that acknowledgement. Once the catalog is usable again, recovery must preserve
the job containing its document. Automatic retirement and the recovery Check
therefore cancel only a freshly proven empty incoming job. Identity matching by
itself does not authorize cancellation of a received document. Fixed UUID/tag,
owner, queue, configured endpoint and connection checks remain required. Missing
or conflicting metadata retains uncertainty. Terminal jobs can still retire
metadata; acknowledged records still preserve the job and retire only metadata.

Withdrawal of the original live request is different authority. Before acceptance,
an actual cancellation signal or parent-channel loss may cancel that request's
identified job even if CUPS already received its document. A polling error alone
cannot grant this authority, and it is never written into a recovery record or
inferred at restart. Once the complete acknowledgement has committed the job,
receipt failure cannot turn it into a withdrawal. No new password request is
allowed during either kind of automatic retirement.

This preserves explicit caller/Request.Close/Lockdown withdrawal while preventing
ordinary failure cleanup from cancelling a received job. A missing final reply
still cannot report submission success or resend the document. If its job already
contains that document, cleanup retains the uncertain record instead of cancelling
it. Parent cleanup after the original worker exits also requires a durable recovery
binding. A Created event only carries a job number, so a client without configured
recovery cannot safely launch a new numeric-only cancellation worker. Preserve its
uncertainty instead. Production delivery already configures durable recovery; the
original worker still performs checked best-effort withdrawal itself.

A later explicit job-management action is needed to cancel or authenticate an
accepted held job; blanket recovery is not that action.

The final candidate passes the actual acknowledged-write-failure regression and
both automatic and explicit recovery after catalog repair. The original request's
signal and pipe-close controls still cancel its received job. Empty-job cleanup,
received lost-final responses, cached/changed protection spaces, protected Iced
recovery and public Print lost replies pass in both appearances. The parent
subprocess test covers eight registered/unregistered outcomes; it establishes
parent policy, separately from the actual scheduler checks.

The source-pinned result records eighteen native replay steps, seventy check
groups and244 observed terminal process identities. Two hundred Print checks are
included in418 serial Portal library checks;55 integration checks,12 Lockdown
checks, the explicit PPD runtime case, strict workspace Clippy and original
whole-workspace compilation also pass. This is selected test execution after
compilation, not an all-workspace execution result. The earlier interrupted build
and obsolete cleanup expectation failure remain retained.
See [scoped evidence](../../.scratch/xdg-portals/print/accepted-job-preservation-evidence.json)
and [checkpoint](../../.scratch/xdg-portals/PHASE59-LIVE.md). No native executable
copies, new dependencies or alternate build graph were introduced.

The held-device investigation also proves a separate missing follow-up: the
current worker exits Accepted before a gated backend rejects credentials, and its
normal record is already gone. Raw CUPS-Authenticate-Job corrects the same UUID
without another document upload. Queue metadata and current Canopa capabilities
can still report no authentication requirement after this late rejection. That
feature remains open; this decision does not implement Iced held-job sign-in.

Print stays opt-in. Full authentication, the eighteen-interface objective,
connection/device/resource/runtime/release gates and existing test failures remain
open. The user prioritizes Canopa Print features before automatic Qt integration.
