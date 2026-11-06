# Explicit protected printer recovery

Status: scoped native verified; full Print authentication and release acceptance remain open.

A disposable actual CUPS/PAM fixture reproduced the original recovery gap. A late
creation login selected a different account, and its durable intent preserved
that account before the scheduler accepted Create. Killing that worker before
its reply left one exact tagged job and an unknown numeric ID in the ledger.
Fresh recovery under the desktop account returned one unresolved record without
network I/O or a password request. The same-account control requested a password
and cancelled only that job. The unrelated control stayed incoming. The failed
baseline assertion remains in `target/portal-print-58/recovery-probe-01`.

An explicit check and automatic retirement now have separate policies. The
private RecoverPrinterJobs operation defaults to unattended. Startup and error
cleanup retain this mode. The parent suppresses interactive challenges during
unattended or cleanup execution, and the worker uses cache-only authentication.
A worker request alone cannot open a password dialog. The Iced recovery action
creates a fresh broker for that check; credentials use the existing private
channel and masked form. They do not become window preferences, public settings,
ordinary output or recovery metadata. Cancelling login stops the explicit batch.

The configured endpoint and encryption preference must match the receipt.
Explicit review keeps its approved HTTP principal separate from the immutable
job owner. Public IPP queries send the recorded requesting owner for visibility
without changing cupsUser or supplying credentials. Unknown IDs require an exact
full tag, owner and queue match. Matching candidates bind their IDs and read-only
UUIDs before a cancellation challenge. Existing durable UUID bindings can be used
directly. Bounded iteration rejects duplicate attributes, duplicate IDs, excessive
jobs or ambiguous data.

Those operation-local bindings survive login and are checked again on the
transmission connection. A previously proven UUID permits privacy-hidden name
and owner fields; visible conflicts still fail. Without UUID, exact visible
identity remains necessary. Cancelled or completed states can be verified without
requiring an empty incoming job. Submission retains its separate empty-job rule.
An accepted receipt still causes only metadata retirement, and live workers
remain protected. No document is sent, and no new job is created during recovery.

The cancellation callback defers authentication inside the checked mutation and
captures the actual challenged resource. CUPS encodes approved credentials in a
fresh protected HTTP context outside that guard. Identity is queried again before
another Cancel. Authentication has a finite budget distinct from the existing
reconnect budget. An unused final password prompt is avoided. The connection
supervisor stays owned through cleanup; no persistent process or runtime profile
is added.

The first linked candidate passed the baseline and a different nonadministrator
login, but failed wrong-once recovery. Successful metadata reads reset the CUPS
password retry count; cancellation can reuse that password without invoking the
callback. Its reconnect is denied by the guard, and the public HTTP status is
then -1 while the new WWW-Authenticate field survives. A bounded GDB observation
confirmed this through public getters. Earlier failed diagnostic receipts remain.

The corrected candidate owns its explicit public IPP Cancel request and resource.
It clears the previous response challenge before the checked action. A new
challenge on the failed action, including a cached-auth reconnect denial, can
request approval outside the guard; a generic reconnect failure alone cannot.
The same fixed job proof is checked again before mutation. Native wrong-once,
third-attempt, lost-reply, permission and separate-namespace checks now pass this
path. Failed first-candidate and diagnostic receipts remain;
its earlier green compile/tests alone did not establish this correction.


Public HTTP/IPP bindings and job identity now have shared modules beside
submission and recovery. Existing submission uses the same fresh-context helper
as before. The IPP iterator and boolean signatures were checked against installed
public headers. CUPS's [job utility implementation](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/util.c)
provides the protocol reference; no private CUPS layout is accessed.

Scoped native checks cover distinct owner/account, unknown IDs, default privacy,
wrong and declined credentials, changed UUID/owner/queue/ID, permission denial,
TLS, multiple records, accepted/busy/legacy work, endpoint replacement and review
closure. Both Iced appearances pass wrong-then-correct, Cancel and review-close
checks. A real sixteen-record batch uses one approval. Accepted jobs, live workers
and unrelated jobs remain protected. Existing file recovery, direct/prepared
printing, GTK/explicit Qt callers, driver/raster controls and cancellation regressions
also pass. The namespace control does not establish arbitrary proxy retargeting
after the last identity check or actual Send namespace replacement.

Visual inspection caught manual review dropping its configuration when launching
the password helper. Forwarding common configuration/default flags fixes that
mismatch; the final light prompt and review use the selected appearance. Only
main.rs changed after the worker matrix. The evidence retains those earlier
executable pins and proves every library/Cargo/vendor input unchanged. Current
Iced flows and the remaining regressions ran on the final executable.

Format, strict workspace Clippy and the original workspace compilation pass.
Socket-enabled compiled checks pass 416 Portal library tests serially, including
198 Print and 12 Lockdown tests, plus 55 integration tests and the explicit PPD
runtime case. The evidence index covers 189 native groups and 989 observed terminal
process identities. See [scoped evidence](../../.scratch/xdg-portals/print/protected-recovery-evidence.json)
and [the phase checkpoint](../../.scratch/xdg-portals/PHASE58-LIVE.md).

The original restricted-socket test failures, an immediate process-exit fixture
check and a later interrupted orchestration process remain recorded. A bounded
exit wait and explicit continuations passed without rebuilding. The orchestration
SIGTERM sender is unknown. The early-TLS Send timing failure remains separately
tracked in [issue08](../../.scratch/xdg-portals/issues/08-early-tls-rejection-observability.md).
Passing repeats do not erase it or authorize retries on a broken connection.
No complete workspace execution green result is claimed; the existing Files and
parallel Portal handoff failures remain. Validation reused root incremental output
and checksum native stores with no binary copies or new dependencies; phase disk
usage finished below its starting value.

Print remains opt-in. Held-job Authenticate-Job, Digest, longer credentials,
connection compatibility, physical/driverless/Flatpak and full packaging/release
acceptance remain required. The full eighteen-interface goal stays active;
automatic Qt integration remains deferred.
