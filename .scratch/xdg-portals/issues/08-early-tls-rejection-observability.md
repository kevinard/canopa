# Preserve uncertainty when an early TLS rejection is not observed

Status: scoped resolved for the selected private runtime; broader connection acceptance remains in issue06

The phase58 regression sequence stopped at `send-worker-17`: the private gateway
recorded an early HTTP401, then closed without consuming the chunked request.
The worker reported streaming failure before any authentication challenge. It did
not resend the document. Exact cleanup cancelled its empty incoming job and left
the unrelated accepted control unchanged. Every observed fixture process ended
and temporary authentication data was removed. This failure remains preserved.

The server generating401 does not prove that the client received a complete
rejection before the connection broke. The original untraced repeat23 passes;
debugger22 with ordinary ignored-SIGPIPE handling passes; valid100-then401
controls19/20 and delayed-close24 pass. The first debugger18 used inferior getter
calls and reached its fixture deadline; debugger21 stopped at SIGPIPE. Neither is
acceptance or evidence that the production runtime stopped on that signal.

No production submission change was made for this observation. Do not infer
nonacceptance from a write error, a pending empty-job query or a response-header
fragment. Continue a document only after an observed definitive rejection and
fresh proof of the same empty job, endpoint and immutable owner. Lost final
acceptance must never cause another document send.

Separate a reliably delivered early rejection fixture from a lost-response
transport fault, then measure exact client results at that seam. Inspect public
CUPS return values and statuses without logging credential bodies. Additional
connection compatibility gates from issue06 remain open. A passing repeat does
not close this issue or erase the original failure. It does not establish a
regression introduced by protected recovery: the streaming Start/Write path was
unchanged in the shared-module extraction, and candidate03 only changes manual
recovery configuration forwarding.

Primary reference: [CUPS2.4.20 request implementation](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/request.c).
Native receipts retain actual binary pins; current continuation25/26 covers the
remaining TLS wrong-once and lost-final cases, both passing. See
`target/portal-print-58/send-transport-diagnosis-01.json` and
`target/portal-print-58/existing-send-continuation-01.json`.

## Phase70 resolution

Controlled public CUPS calls now separate a complete observed rejection from a
reply sent by the server but lost before the client parses it. A writable/drained
TLS peer yields401. A closed peer yields a write error without a challenge even
when it sent a complete401. The actual worker continues only the first case and
stops without another send for the lost reply. This does not reconstruct the
precise scheduling of historical send-worker-17 or erase its failed receipt.

The new partial-header control exposed a separate upstream CUPS2.4.20 loop:
`httpUpdate` preserves cached401 on EPIPE in POST_RECV, so request setup cannot
finish. The maintained exact public-request regression is red before the fix.
The canonical eight-file overlay now keeps the read error and propagates it
before request setup can authenticate/reconnect. All eight maintained cases pass
twice on client903; the actual worker passes full/drop/status-prefix/challenge-
prefix plus wrong-once/lost-final controls. Incomplete replies fail without a
prompt or another send and retire only the exact empty owned job.

The private Digest fixture also now verifies its Expect gate before deciding
whether corrected credentials are valid. Two old-gate failures are preserved;
that fixture correction is separate from the production CUPS header fix.

The installed system library is unchanged; its slower malformed-reply behavior
remains bounded by the existing outer worker supervision. Use the explicitly
built/packaged fixed client for the measured early-return behavior. Broader
connection, device/Flatpak and release/deployment gates remain open. See
[ADR0106](../../../docs/adr/0106-print-incomplete-http-replies.md),
[the evidence](../print/early-reply-evidence.json) and target/portal-print-70/.
