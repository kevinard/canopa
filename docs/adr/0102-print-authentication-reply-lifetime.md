# One lifetime for printer authentication approval and its private reply

Status: scoped native verified; full Print gates remain. Print stays opt-in.

The authentication helper returns an approval bit through ordinary stdout and
credentials through a separate inherited socket. The backend previously bounded
only the ordinary exchange by 90 seconds. After approval and successful helper
exit, it read the private length, body and final EOF outside that timeout and its
direct cancellation scope.

A controlled helper reproduces the gap through the real Print controller,
Production adapter, broker, prompt and worker supervisor. A truncated prefix
followed by EOF is refused immediately. With the same prefix and an additional
writer retained after helper exit, the request remains pending after 95 seconds.
Closing that writer immediately completes the failure. The stand-ins do not
establish a failure in normal Iced behavior, an actual scheduler or a physical
printer. They establish the missing lifetime bound at the private reply boundary.

The prompt now applies one 90-second deadline to approval, helper exit and the
entire private frame, including EOF. Caller withdrawal or loss cancels the same
interaction. The deadline is never restarted after approval. Dropping an
incomplete interaction closes its descriptors and clears any owned secret frame
through the existing secret owner. Frame limits, typed credential validation,
request-local cache ownership and explicit Iced consent are unchanged.

Regression checks retain a duplicate writer after helper exit. They send a partial
length, a partial body or a full frame without EOF, then withdraw or disconnect
the caller. Drain of the writer's socket queue proves the parent has passed the
ordinary approval and begun reading the private reply. A complete frame with EOF
also verifies successful decoding. The native faulty-helper replay measures the
actual production deadline without a shorter test-only production timeout.

The original reproducer now completes at 90.2 seconds while the controlled writer
is still open. Both new regression tests pass, including six partial/unclosed
reply withdrawal variants. All 437 Portal library tests, 55 integration tests and
the PPD runtime case pass, as do strict workspace Clippy and compilation with the
original default workspace graph. Four actual Iced invocations cover file output
and wrong-password correction over TLS/PAM in both appearances. All 94 unique
observed PIDs in candidate native receipts are terminal; private data is removed.
See [the scoped evidence](../../.scratch/xdg-portals/print/authentication-reply-lifetime-evidence.json).
Full workspace execution was not repeated for this local fix; the earlier ten
Files integration failures remain open.

See [the current investigation](../../.scratch/xdg-portals/PHASE65-LIVE.md),
[request-owned authentication](0094-request-owned-printer-authentication.md) and
[remaining authentication work](../../.scratch/xdg-portals/issues/04-authenticated-printing.md).
Outer worker supervision, connection compatibility, runtime packaging and full
Print/device/release acceptance remain separate gates. Automatic Qt routing stays
deferred, and the full eighteen-interface goal remains active.
