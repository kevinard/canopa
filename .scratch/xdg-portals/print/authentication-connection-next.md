# Remaining Print connection, supervision and runtime work

The user prioritizes Canopa Print. Automatic Qt print-dialog routing is deferred.
For current phase71 binary, tests and stock-filter paper/appearance results, read
[PHASE71-LIVE.md](../PHASE71-LIVE.md),
[ADR0107](../../../docs/adr/0107-exact-paper-and-print-appearances.md) and
[the current scoped evidence](paper-appearance-evidence.json). The binary/source
pins below are historical phase70 evidence; resolved connection checks need not
be repeated solely because the binary has changed.
The subsequent [public paper evidence](public-paper-evidence.json) also passes
standard frontend/normal backend/Iced prepared stamped PDFs in both appearances.
The [maintained receiver evidence](receiver-maintenance-evidence.json) now closes
the receiver source/configuration/loader pairing follow-up. Advance the remaining
advanced-device and connection gates using its current checksum-bound artifact.
Print remains opt-in and the full eighteen-interface goal is active. Phase70 is
sealed; read [PHASE70-LIVE.md](../PHASE70-LIVE.md),
[ADR0106](../../../docs/adr/0106-print-incomplete-http-replies.md) and
[the early-reply evidence](early-reply-evidence.json).

## Current source and evidence

The normal Rust binary remains
`fdfa257f96b1b5234f2a20458f77e068d142ae3d4279745b59c756390e69bf53`.
The selected CUPS client is now
`903123580ada3470849a7c856ea56ab0d7405f0133ed3d11acbcd860cc01fb02`.
The canonical eight-file patch keeps incomplete HTTP headers as a read error and
propagates that error before request authentication/reconnect. Only http.c and
request.c differ from the previous client. All 1,301 inputs and an official-archive
materialization, LICENSE and NOTICE are verified. The stable cache rebuild takes
3.01 seconds; no Rust graph or native executable is added.

The exact public-request regression is red on client49 and passes eight cases
twice on client903. Six actual-worker scheduler cases, 77 HTTP/77 TLS authentication
cases and nine packaging groups including 12 packaged metadata cases pass. All 188
recorded candidate identities are terminal and absent on a host audit; private
TLS/authentication data is removed. A server sending 401 still does not prove the
client observed rejection: write errors remain unconfirmed and never permit a
resend. The old fixture's Digest Expect gate is corrected independently.

Strict workspace Clippy5.79s and original workspace/all-target compilation1.92s
pass. Portal437/55 pass in the full default-parallel run. The same ten Files
integration failures remain; its previous video failure does not repeat. Settings,
CLI, configuration and vendor poststages pass separately. Initial sandbox systemd
socket denial and the unchanged host continuation are retained; no completed test
is repeated or feature graph changed. No full workspace green result is claimed.
Source guards distinguish the actual native inputs from later guide/ADR/context
metadata. Older binary/client receipts retain their historical pins.

Phase64 established real idle90s and private reply120s deadlines, partial reply
EOF/cancellation, file-output fresh-consent and delivery in both appearances, and
compiled storage/admission with explicit worker observations. Phase65 reproduced
and corrected a postapproval timeout gap: ordinary approval, helper exit and the
whole private frame/EOF now share one90-second deadline and caller lifetime. The
same faulty-helper case completes at90.2s with its retained writer still live,
versus no response after95s before the fix. Six withdrawal variants and valid EOF
pass. Four actual Iced invocations revalidate file delivery and wrong-once TLS/PAM
protected recovery in both appearances.94 unique observed candidate PIDs are
terminal and private data removed. See [ADR0102](../../../docs/adr/0102-print-authentication-reply-lifetime.md).

The postapproval incomplete-reply investigation is resolved; do not investigate it
again as an unmeasured seam. The faulty-helper stand-ins establish the real parent
boundary, not normal Iced behavior or actual printing. File and recovery fixtures
have separate actual Iced/scheduler scopes.

Phase66 reproduced quoted-field failures, then fixed challenge decoding and
Digest serialization in the canonical seven-file CUPS overlay. All 48 original
positive cases pass. The maintained tool passes 77 cases over loopback HTTP and
77 over private TLS, including maximum quoted/UTF-8 values and refusal controls.
Actual quoted SHA-256 Iced file flows pass in both appearances; quoted-account
Create and TLS Send-only correction pass through the independent gateway.
All 343 unique candidate native PIDs are terminal and private data is removed.
The quoted-field investigation is resolved; do not repeat it as an open seam.

Phase67 packages the optional private runtime and original upstream notices from
its verified build receipt. Full eight-binary archive/smoke, metadata-worker
packaged-image and relocated-prefix normal-backend/Iced checks pass. The latter
uses no runtime-selection environment override and records30 terminal process
identities. Source-only tooling changes preserve the current Rust/client hashes;
the additional packaging input guard is target/portal-print-67/source-guard-final-01.json.
The Ubuntu release workflow is configured but has not run locally. See
[ADR0104](../../../docs/adr/0104-packaged-private-print-runtime.md).

Phase68 verifies the actual parent60/300-second budgets and10-second stop grace
through public Client APIs, plus42 retained-parent rounds of authentication,
withdrawal, dropped futures and errors. Every round returns to its11FD baseline,
one image and no children. All114 unique candidate observed identities are
terminal, private data removed. These are controlled child/credential protocols,
not actual CUPS loading, Iced or device checks. No product Rust/Cargo/runtime edit.
The phase68 guard target/portal-print-68/source-guard-01.json adds the three fixture
sources; its only prior-guard difference is the updated Print guide. The one17MB
fixture is losslessly compressed once by checksum after validation. See
[the supervision guide](../../../docs/print-supervision.md).

Phase69 resolves the staging handoff EOF failure. A retained CLOEXEC copy
reproduces it in the actual normal worker; explicit producer write completion
fixes it while copies remain live. The maintained regression is red before/green
after; the original debugger perturbation also passes. Six native file-client
groups pass for held/closed-copy controls, plus three public frontend/Iced groups
per appearance. All96 observed candidate identities are terminal/privatecleanup.
Receiver rejection of extra packets, open writers and allocation mismatch stays
strict. The historical phase57 fork identity is unknown; do not invent a trace.
See [ADR0105](../../../docs/adr/0105-print-staging-stream-completion.md).

## Next concrete work

1. Finish the existing [driverless/custom media](../issues/02-custom-paper-device-precision.md)
   and [advanced device delivery](../issues/03-advanced-device-delivery.md) gates.
   Installed ippeveprinter, ippfind, ipptool and cupsfilter are available; prior
   failed receiver attempts and exact downstream limits remain evidence.
2. Establish actual Flatpak identity/access and release/deployment acceptance.
   The native host has /dev/fuse, but no Flatpak CLI or libostree/libflatpak.
   The sandbox's device view hides FUSE; do not reuse that view as a host blocker.
   Existing synthetic namespace checks do not establish this gate. Prepare a bounded
   concrete environment before proposing system changes. Runtime packaging/NOTICE
   and parent deadline/FD checks are locally verified; do not reopen them merely
   because another turn begins. Keep Print opt-in until required gates pass.

The staging EOF and selected-runtime early-reply investigations are resolved. Preserve its baseline failures and
current scoped result; do not reopen it solely because another turn begins.

The file tests establish credential disposal through fresh consent and no printer
I/O during prepared file delivery; they do not inspect every allocator byte. That
scope distinction is not an additional allocator-wide feature requirement.

The independent Digest gateway verifies Digest before actual Basic/PAM forwarding;
it is not native cupsd Digest/PAM. The namespace device executor adapter proves
protocol behavior, not physical or installed-executor acceptance. The staging and selected-runtime early-TLS investigations are resolved. Six
other experimental interfaces and broader connection acceptance remain open. Phase66's required workspace run retains the existing ten Files integration
failures and adds one video-playback library failure. The exact compiled video
case passes an isolated retry from its crate directory; no cause or fix is
established. Settings, CLI, configuration and vendor post-stages pass separately
after the Cargo failure. The authentication self-test also passes independently.
Full workspace acceptance remains open.

Storage stays bounded: root incremental/default graph and checksum stores only,
no cache purge or per-evidence ELF copies. Phase66 retains one new 841160-byte
client image, with about 15 MB net growth at seal. The old native harness and current normal binary are compressed
losslessly once per checksum, not copied into every fixture directory. Preserve
source-only replay, the failing baseline and concise receipts.
