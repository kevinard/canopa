# Quoted printer authentication fields

Status: scoped native verified; full Print gates remain. Print stays opt-in.

An independent byte-oriented HTTP authentication oracle reproduces incorrect
quoted-field handling in the optional CUPS client. Of 48 positive worker cases,
30 fail with the previous image. Digest usernames, realms, nonces and opaque
values containing quotes or backslashes fail authentication. Basic can authorize
while exposing an incorrectly decoded realm to the private consent prompt.
Plain controls and published MD5/SHA-256 reference vectors pass independently.

Extend the canonical CUPS source overlay to seven files. Decode quoted pairs in
challenge fields and when exposing a realm through the public CUPS API. Escape
quotes and backslashes when serializing Digest fields, while hashing the original
decoded bytes. Reject unrepresentable or truncated output. Preserve bounded
credential capacities, the existing capability version and process isolation.
The Rust implementation and dependency graph do not change.

This follows [RFC7616 section 3.4.5](https://www.rfc-editor.org/rfc/rfc7616#section-3.4.5)
and [RFC9110 section 5.6.4](https://www.rfc-editor.org/rfc/rfc9110#section-5.6.4).
The maintained oracle has three published digest vectors, two quoting checks,
seven malformed-input checks and two Basic scheme-case checks. It uses an
independent byte parser and does not record authorization headers or passwords.

The same 48 positive worker cases now pass. The maintained tool passes 77 cases
over plain loopback HTTP and 77 over private TLS; a final plain run verifies its
final source. These cover Basic, MD5, SHA-256 and legacy Digest, quoted and UTF-8
values, maximum supported lengths and refusal controls. An old-image subset
retains 20 failures out of 23 cases. Its quoted fake-Basic controls emitted Digest,
not Basic; this evidence does not establish a Basic downgrade. The candidate
sends no authorization for those unsupported-qop offers.

Actual frontend/Iced runs in both appearances verify quoted SHA-256 fields,
withdrawal, fresh consent after selecting file output and prepared four-page PDF
delivery without a printer job. Separate protected Create and TLS Send-only
wrong-password correction checks use a synthetic account containing a backslash
and quote. Their independent Digest gateway forwards verified identities to an
actual private CUPS Basic/PAM scheduler; this is not native cupsd Digest/PAM or
physical-device acceptance. All 343 unique candidate PIDs recorded across the
native receipts are terminal, and private authentication data is removed.

The seven-file patch independently materializes against all 1301 pinned upstream
source files, with original license and NOTICE verification. One 841160-byte
client image is retained by checksum. The original incremental workspace graph
is reused, with no per-fixture executable copies or cache purge.

The required workspace check passes all 437 Portal library and 55 integration
tests, but remains non-green: the existing ten Files integration failures and one
Files video-playback library failure occur. The video case passes an isolated
compiled retry from its crate directory; no cause or fix is established. Settings,
CLI, configuration and vendor post-stages pass separately after Cargo's failure.
The maintained authentication self-test also passes separately. See
[the scoped evidence](../../.scratch/xdg-portals/print/quoted-authentication-evidence.json)
and [the remaining work](../../.scratch/xdg-portals/print/authentication-connection-next.md).

Runtime packaging/NOTICE, outer supervision and repeated authenticated resource
use, driverless/device and Flatpak acceptance remain open. Automatic Qt routing
stays deferred, and the full eighteen-interface goal remains active.
