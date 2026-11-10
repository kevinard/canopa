# Reject incomplete printer HTTP replies

Status: selected private runtime and actual worker verified; workspace retains
ten existing Files integration failures. Print remains opt-in and broader
device/deployment gates remain.

A printer generating HTTP 401 does not prove that the client observed it. A
controlled TLS peer can send a complete rejection and close before the next
document write. The public `cupsWriteRequestData` then returns an error without
exposing a challenge. Keeping the connection writable while discarding the
rejected stream instead returns401. Canopa must preserve the first outcome as
unconfirmed; neither socket readability nor an empty job authorizes a resend.
This explains the compatibility boundary without reconstructing the precise
scheduling of the preserved phase58 failure.

The investigation also reproduced a separate CUPS 2.4.20 defect. After a truncated
status line or challenge, `httpUpdate` returns the cached 401 on EPIPE while
remaining in `HTTP_STATE_POST_RECV`. `cupsSendRequest` loops on that state. Its
public API regression reaches a three-second child deadline on the old private
client. The actual worker reaches its raw fixture deadline with one empty job,
one document attempt and no credential prompt. These failures remain retained.

The canonical CUPS overlay now changes two additional behaviors. `httpUpdate`
does not hide a header-read failure behind its cached status. `cupsSendRequest`
propagates failure to read the remaining headers before authentication or
reconnect. Both changes are needed: returning an error from `httpUpdate` alone
would still let request setup act on its earlier 401. The complete patch now
contains eight upstream files; the previous seven-file authentication fixes and
original LICENSE/NOTICE remain intact. All 1,301 prepared inputs and a fresh
official-archive materialization match the declared recipe.

The maintained [early-reply regression](../../scripts/test-print-early-replies.py)
uses the same public request API called by `cupsStartDestDocument`. The peer
drains the initial IPP chunk before publishing its rejection, so an earlier
failed write cannot hide the header-parser failure. A complete rejection reaches
the refusing synthetic password callback once. Status-only, challenge-prefix and
missing replies return errors without calling it. Separate 8 MiB synthetic-write
cases distinguish observed rejection from lost response. No scheduler, document,
password, Iced window or host configuration is involved at this seam.

The unchanged regression is red on client `49cedda4` and passes all eight cases twice on
client `90312358`. The actual normal Rust worker also passes six private scheduler cases:
complete early rejection, dropped reply, both truncated forms, wrong-password
correction and lost final acceptance. Only a complete rejection permits fresh
identity checks and continuation on the same job. Incomplete replies fail and
retire only the exact empty owned job. A lost final acknowledgement preserves its
single accepted document without another send. The unrelated accepted control
and immutable creation owner are preserved.

The private Digest gateway needed its verifier at the Expect gate as well as the
ordinary request handler. Two preserved baseline failures identify that fixture
bug; correcting only that boundary permits the full authenticated control. This
gateway verifies Digest before forwarding Basic to private CUPS/PAM. It does not
establish native cupsd Digest/PAM, normal Iced or physical-device acceptance.

The candidate passes 77 independent authentication cases over HTTP and 77 over
TLS, plus nine packaging/refusal groups including 12 actual-worker packaged-image
cases. It reuses the stable CUPS cache and unchanged Rust executable. Only two C
objects and the small client are rebuilt; there is no new Cargo graph or copied
native executable. Source, receipts and the previous immutable client remain
available in the checksum store. See
[the evidence](../../.scratch/xdg-portals/print/early-reply-evidence.json).

Strict workspace Clippy and original workspace/default/all-target compilation
pass. Portal passes 437 library and 55 integration tests in that workspace run.
The ten existing Files integration failures remain; its earlier video failure
does not repeat, without a code fix or cause claim. The remaining settings, CLI,
configuration and vendor stages pass separately after Cargo reports failure.
No full workspace green result is claimed.

The installed system CUPS library is unchanged. System fallback can retain its
slow malformed-reply behavior inside the existing outer worker budget. The fixed
client is the explicitly built/packaged runtime; its image checksum identifies
this change. Print advertisement, broader connection compatibility, driverless
and physical devices, actual Flatpak, release-workflow execution and deployment
remain separate. The full eighteen-interface goal stays active; automatic Qt
integration remains deferred.

Update, 2026-11-10: the `cups-authentication` overlay recipe was removed with `vendor/`. The incomplete-header change is now the `canopa/http-incomplete-headers` topic of [kevinard/cups](https://github.com/kevinard/cups), with a tracking PR in the fork; the client, IPP backend and receiver are all built from its pinned `canopa/main` commit (`packaging/forks.json`). Bundles carry the original LICENSE/NOTICE and a summary naming the fork commit instead of patch files. See ../vendor-maintenance.md#cups-client-ipp-backend-and-receiver.
