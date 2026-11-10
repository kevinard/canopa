# Process-private printing authentication runtime

Status: scoped runtime/capacity native acceptance verified; full Print gates remain.

Canopa's isolated Rust/Iced printing workers use public CUPS APIs. Native Digest
probes against the installed client library accept explicit MD5 and SHA-256,
including TLS, UTF-8 passwords and ordered device fields. Valid default-MD5
challenges without an algorithm are rejected because the library emits a legacy
response without qop. Its legacy response omits the supplied opaque value.
A separate check accepts MD5 despite a private DigestOptions DenyMD5 setting.
The released source parses that setting but never copies it to active globals.
The original failed receipts remain in the phase61 investigation.

The same client has credential-capacity boundaries below Canopa's typed limits:
its Basic encoder cannot preserve a 190-byte username:password value, its cached
user:password and Digest copy are bounded at 255 bytes, and its global user name
is 64 bytes. The phase61 Rust cap refused oversized combined values. A 54-case independent
public-API comparison reproduces those limits and verifies
that the candidate preserves 127-byte usernames and 1023-byte ASCII/UTF-8 passwords
for Basic, MD5 and SHA-256. This deliberately bypasses the then-unchanged Rust cap;
it is not production long-credential acceptance.

Prepare one optional client-library source overlay in root vendor/, using the
official pinned release archive, original licence/NOTICE and a reviewable patch.
Keep the application's Rust dependency graph unchanged. Build only libcups.so.2,
with the same stable source/object directory, one compiler job and bounded storage.
Native checks select it only for the Rust worker process and verify its mapping.
The private scheduler and configuration tools retain their system library.

The candidate fixes negotiated Digest qop/default MD5, legacy opaque and active
DigestOptions, and increases bounded client buffers for 127-byte usernames and
1023-byte passwords. Input/header truncation is refused. It changes private CUPS
structures, so loading it into a system daemon or a module compiled against
another layout is invalid. Rust continues to use opaque public API handles.
No system installation or packaged route is changed.

Runtime selection now freezes one sealed image for the backend lifetime. The
optional bundle lives at PREFIX/lib/canopa/print/libcups.so.2 beside PREFIX/bin; an
explicit operator override is CANOPA_PRINT_CUPS_LIBRARY. An absent bundle selects
the system soname. A present invalid selection fails rather than silently falling
back. The main process copies at most 16 MiB and never dlopens CUPS. Its Iced
metadata helper receives that same snapshot, and file operations inherit no CUPS
image. CUPS workers retain both image and Library until process exit because of
CUPS thread-local destructors. No ambient LD_LIBRARY_PATH selection is introduced.

The overlay adds canopaCupsAuthenticationVersion() = 1. Only the isolated
worker checks it and maps the recognized version to 127-byte usernames, 1023-byte
passwords and 1151 combined bytes. Missing/unknown capabilities fail. System CUPS
keeps 64-byte usernames and 189 combined bytes; oversized input is rejected before
cupsSetUser can truncate it. A typed private policy is part of server-scope equality
and survives sealed prepared handoff, so another capacity cannot reuse approval.
The parent refuses extended challenges from a system-selected worker. Byte counts
apply to UTF-8; queue field rules are unchanged.

Executable memfd selection requests MFD_EXEC, with EINVAL fallback for old kernels.
An enforced no-exec policy remains a visible selection failure; see the
[kernel interface](https://docs.kernel.org/userspace-api/mfd_noexec.html).

Secrets must remain bound to the request's endpoint/protection space and queue;
file output must release unrelated printer credentials. Prepared grants, settings,
arguments, ordinary output and recovery records must never contain secrets.
An absent or unverified compatibility runtime must preserve the conservative
behavior. Do not infer capability from the CUPS version string alone.

Current candidate evidence includes 52 actual Rust-worker Digest cases across
MD5/SHA-256/legacy, TLS, refusal, private-channel failures, default algorithm,
opaque, nonce rotation, wrong-once, DigestOptions and negotiated qop/refusal.
All run against private services with synthetic credentials. These are worker/gateway protocol checks,
not Iced, actual Digest/PAM, physical printing or runtime distribution acceptance.
The normal backend stays opt-in. Create-only/Send-only Digest, protected recovery,
held jobs, Iced/frontend flows, resource/timeouts and previous Print regressions
still need coverage on the eventual selected runtime. Full Print and the other
experimental interfaces remain active; automatic Qt integration is deferred.

The integrated candidate passes 435 serial Portal library tests, 55 integration,
PPD runtime and strict workspace Clippy. Its 120 actual-worker cases include
Basic/MD5/SHA-256, TLS, maximum ASCII/UTF-8 values, conservative fallback and
missing/unknown capability refusal. Four complete Iced/public runs cover Basic and
SHA-256 in both appearances, including actual GTK and explicit Qt prepared callers,
wrong-once, direct/prepared printing and withdrawal. They preserve six expected
pages and exact private cache handoff. Two reduced replays verify masked long-field
rendering after asynchronous paste redraw. Seventy mapping/descriptor observations
keep CUPS out of backend/metadata/credential UI processes and its image descriptor
out of credential helpers. A separate actual-parent handoff check proves source
replacement cannot change the sealed image. All 808 observed identities are terminal.

The required original workspace/default/all-target/no-fail-fast run still fails
only in Files integration: the same exact ten names as phase61. Remaining settings,
CLI, configuration and vendor runtime checks pass independently. Full workspace
acceptance remains open. See [integrated evidence](../../.scratch/xdg-portals/print/selected-authentication-runtime-evidence.json)
and [the original client candidate](../../.scratch/xdg-portals/print/authentication-runtime-evidence.json).

Selected-runtime protected operations now pass a further 49 native invocations:
Create-only/Send-only, retries/TLS/lost replies, explicit recovery with a distinct
approved actor, and same-UUID held-job release without another document. Changed
source/owner/queue/server/requirements and replacement jobs refuse mutation. Ten
actual Iced invocations include both appearances and 140 held-review command steps,
with 24 production isolation observations. All 431 observed identities are terminal;
compiled inputs and client image are unchanged. See
[protected-operation evidence](../../.scratch/xdg-portals/print/selected-protected-print-evidence.json).

The independent MD5/SHA-256 gateway forwards only verified identities through actual
CUPS Basic/PAM; it does not establish native cupsd Digest/PAM. The namespace device
executor remains protocol-only. Prompt/deadline/resource/file-output lifetime,
broader connection/device/package and physical-printer acceptance remain open.

See [the overlay](../vendor-maintenance.md#cups-client-ipp-backend-and-receiver),
[current integration](../../.scratch/xdg-portals/PHASE62-LIVE.md) and
[full authenticated printing](../../.scratch/xdg-portals/issues/04-authenticated-printing.md).
Protocol references: [RFC7616](https://www.rfc-editor.org/rfc/rfc7616.html) and
[the pinned official CUPS release](https://github.com/OpenPrinting/cups/releases/tag/v2.4.20).

The current seven-file overlay additionally fixes measured quoted-pair decoding
and Digest serialization. Phase66 uses a new checksum-pinned client image with
the unchanged Rust binary; the preceding receipts retain their historical image
pins. See [quoted-field acceptance](0103-quoted-printer-authentication-fields.md)
for current validation, including the non-green workspace result and separately
passing post-stages.

Update, 2026-11-10: the `cups-authentication` overlay recipe was removed with `vendor/`. Each authentication change is now one of the `canopa/auth-*`, `canopa/digest-*` and `canopa/http-incomplete-headers` topics of [kevinard/cups](https://github.com/kevinard/cups), with a tracking PR in the fork; the client, IPP backend and receiver are all built from its pinned `canopa/main` commit (`packaging/forks.json`). Bundles carry the original LICENSE/NOTICE and a summary naming the fork commit instead of patch files. See ../vendor-maintenance.md#cups-client-ipp-backend-and-receiver.
