# Bound the private printer credential reply after approval

Status: scoped-verified

A controlled helper returns approval and exits after writing only two bytes of
the private frame length. A separate controlled writer keeps that socket open.
The real Print controller, Production adapter, broker, prompt and supervisor leave
the request pending beyond its 90-second prompt budget. Releasing the writer
immediately completes error response 2. The EOF control completes immediately.
Both cases clean up all recorded processes and private data.

Evidence: `target/portal-print-65/retained-prefix-01/summary.json`,
`target/portal-print-65/eof-prefix-01/summary.json` and
`target/portal-print-65/investigation-01.json`. The reproducer supplies approval,
worker and password stand-ins around the actual Rust parent and snapshot worker.
It is not evidence that the normal Iced helper forks or normally produces an
incomplete reply, nor actual CUPS/device acceptance.

The candidate places ordinary approval, helper exit, private length/body and EOF
inside one 90-second timeout and caller lifetime. No public knob, dependency,
credential representation or GTK implementation is added. Regression coverage
must prove withdrawal after the parent has begun its private read, and successful
decoding of a valid complete reply. The native failing reproducer must pass with
the same actual deadline. Revalidate normal Iced flows in both appearances.

Follow [PHASE65-LIVE.md](../PHASE65-LIVE.md) for execution state and
[ADR0102](../../../docs/adr/0102-print-authentication-reply-lifetime.md) for the
boundary decision. Keep the parent authentication issue and full portal goal open
until their remaining gates are satisfied.


The fix passes the original faulty-helper replay: response2 at90.1966 seconds
while the retained writer is still live, compared with no response after95.0119
seconds before the fix. EOF controls pass on both versions. Two new tests cover
six postapproval withdrawal/caller-loss variants and a valid complete reply.
437 Portal library/55 integration/PPD1, format, strict workspace Clippy and the
original workspace/default/all-target compilation pass. Four Iced invocations
cover prepared file output and TLS/PAM wrong-once recovery in both appearances.
All94 unique observed candidate PIDs are terminal and private data is removed.
See [evidence](../print/authentication-reply-lifetime-evidence.json).

No vendor/dependency, public format or appearance changes were needed. Source
patch and exact guards are retained with the failed baseline; the two checksum
archives are losslessly verified. The root incremental graph was reused and net
storage at seal is about78MB lower than at phase start. Full Print and release
acceptance and the existing ten Files failures remain outside this scoped result.
