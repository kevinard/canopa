# Explain the parallel staging-handoff EOF failure

Status: scoped-verified

Candidate03 phase57 formatting/strict workspace Clippy/original build and195 Print
checks pass. The subsequent default-parallel Portal library run fails only
`print::file_target::publication::handoff::tests::staging_handoff_retains_live_authority_for_both_new_and_replaced_outputs`
at handoff/tests.rs29: Prepared::receive returns EAGAIN (WouldBlock). The same
unchanged exact test passes in isolation, and all413 library tests pass serially
with native local-socket permissions. Preserve worker-03.json as failed; the
continued receipt is separate. These results do not prove the race's cause or
establish default-parallel acceptance.

Ranked hypotheses: a concurrent fork briefly inherits the sender before CLOEXEC;
a distinct handoff race; submission changes unexpectedly affect the handoff.
The fixture sends descriptors, drops its sender and immediately expects EOF on a
nonblocking receiver. No production wait/policy has been changed. A deterministic
regression should distinguish shared inherited senders from corrupt/missing
packets. Explicit write shutdown in the fixture may establish EOF despite a
concurrent inherited descriptor, but verify that hypothesis before changing it.
Do not mask an open-writer rejection by adding sleeps or changing production
nonblocking checks. Add no native executable/cache graph for this investigation.

Evidence: target/portal-print-57/worker-03-portal-library.out (412/1/20ignored),
continuation-03-native-handoff-isolated.out and
continuation-03-native-portal-library.out (413/0/20ignored), all from the same
source/binary. The first continuation without native permissions failed private
socket/DBus cases; that sandbox failure is separate and retained.

## Resolution

Phase69 reproduces the exact WouldBlock twice in the existing harness by retaining
only the sender at its close. The socket has CLOEXEC. A separate maintained
caller-seam regression through the actual normal setup worker fails with the same
staging error while a forked CLOEXEC copy remains live. The closed-copy control
passes all six native file-client groups. A diagnostic prototype adds explicit
write completion after the unchanged worker response and passes those groups with
five copies still live. This demonstrates the mechanism and production exposure;
the exact historical phase57 fork remains untraced.

The actual setup producer now shuts down its write half after the descriptor
bundle is sent. Positive unit fixtures explicitly complete their stream while a
CLOEXEC clone stays live. Open writers still return WouldBlock; extra packets
and mismatched allocations return InvalidData. Receiver validation is unchanged,
with no waits or sleeps. The maintained regression is red before the fix and green
after it; the original debugger perturbation also passes after the fix.

Format/strict workspace Clippy/original graph build, default-parallel Portal437,
integration55 and PPD1 pass. Actual held/closed-copy native controls pass six groups
each; public frontend/Iced file flows pass three groups per appearance, including
prepared four-page file delivery and no printer job. All96 observed candidate
identities in the retained native receipts are terminal and private data is
removed. The old unique harness is archived once by checksum; no new native
executable/cache graph is added. No full workspace execution claim is made.

See [ADR0105](../../../docs/adr/0105-print-staging-stream-completion.md),
[evidence](../print/staging-completion-evidence.json) and
[execution state](../PHASE69-LIVE.md). Explicit protocol completion and a retained
writer at the actual caller seam prevent the hidden dependency on final descriptor
closure. No new module/interface seam was required. Print remains opt-in and the
full portal goal stays active.
