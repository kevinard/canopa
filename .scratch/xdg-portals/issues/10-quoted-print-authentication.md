# Preserve quoted printer authentication fields

Status: scoped-verified

The previous selected CUPS image fails 30 of 48 independently checked positive
worker cases involving quoted usernames, realms, nonces and opaque fields. Basic
also exposes an incorrect private prompt realm for quoted challenges. Published
Digest vectors and plain controls pass. Preserve that baseline and fix the
canonical overlay rather than adding an application-local CUPS copy.

The seven-file overlay now decodes quoted pairs and serializes escaped Digest
fields without changing the bytes hashed. Bounded output construction refuses
truncation. All 48 cases pass; the maintained tool passes 77 cases over loopback
HTTP and 77 over private TLS. Actual Iced flows pass in both appearances, and
quoted-account Create and TLS Send-only correction pass against the private
Digest-to-Basic/PAM gateway. This does not establish native cupsd Digest/PAM,
physical printing or distribution acceptance.

All 343 unique candidate native PIDs are terminal and private authentication data
is removed. Original upstream source, license, NOTICE and canonical patch checks
pass. The required workspace run remains non-green with ten existing Files
integration failures and a video-playback library failure that passes its isolated
compiled retry; no Files cause or fix is claimed.

See [ADR0103](../../../docs/adr/0103-quoted-printer-authentication-fields.md),
[evidence](../print/quoted-authentication-evidence.json) and
[execution notes](../PHASE66-LIVE.md). The parent authentication issue and full
portal goal remain open. Continue with
[the remaining work](../print/authentication-connection-next.md).
