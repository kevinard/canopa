# Phase61: Digest authentication compatibility

Status: scoped client-library candidate verified; production integration pending.
All builds, native probes and workspace checks are terminal. No pending approval,
blocker, commit or PR. Full 18-interface goal stays active: 11 packaged, six
experimental, Print opt-in. User prioritizes Canopa Print; automatic Qt deferred.

## Verified candidate

The optional `cups-authentication` source overlay pins the official OpenPrinting
2.4.20 archive (8,164,882 bytes), SHA256
`ab4d9cd7f3e58060091d2b24972223d6401675f11c49b65abf4f6ef31dea22ff`.
All 1301 source files and canonical LICENSE/NOTICE are verified. The 154-line,
five-file patch fixes negotiated Digest qop/default MD5, legacy opaque, applied
DigestOptions and bounded username/credential/Base64/hash buffers. The builder
compiles only libcups.so.2, one job, GnuTLS, stable source/object cache, no install.
Configure 22.01s/library 13.01s. Six source/notice/materialization tests pass.

Library SHA256:
`fe5d42010869eda4d1034a3fb0f3e8e99c65c7365602072b7e82e829188a60be`.
One checksum-verified shared archive retains its 841,064 bytes before future
rebuilds; no per-evidence binary copies. Its changed private CUPS layout is valid
only in isolated clients using opaque public APIs. Never inject it into installed
cupsd, tools, filters or modules. Probes verified worker mapping and separately
verified that the private scheduler retains the system library.

Candidate matrix01 and qop-controls02: 13 invocations, 52 actual Rust-worker cases
pass, covering SHA256/MD5/legacy, default algorithm, opaque, DenyMD5, nonce rotation,
wrong-once, TLS, auth selection from a qop list, auth-int-only refusal and original
private-channel failures. Long-credentials02: 54 independent public-CUPS API
comparisons pass, reproducing baseline boundaries and verifying candidate 127-byte
users/1023-byte ASCII or UTF8 passwords (1151 combined). This helper deliberately
bypasses Rust's unchanged 189-byte cap. All 120 observed identities are terminal;
private auth data removed. These are gateway/worker protocol and separate raw API
checks, not Iced/PAM/physical/runtime distribution acceptance.

No Rust, manifest or lockfile was changed in phase61. Original compiled inputs
are verified unchanged except the exact new overlay provenance entry. Rust binary
remains phase60 worker07:
`1373a3ac4764b7be1fa675a672718bc41465e44c6a4bf159d622585bf5cc0e91`.
Normal backend still loads system libcups and retains its 189-byte combined cap.

## Validation and retained failures

Required workspace01/52763 terminated at a stale generated inventory hash after
vendor/tooling, format and strict workspace Clippy passed. All 650 inventory rows
were identical; only the current lockfile SHA header was regenerated. The original
workspace/default/all-target continuation42050 used the same incremental cache,
compiled in 2.57s and ran all test executables with --no-fail-fast (862.35s).
Portal library 425 and integration55 pass. The failing target is Files integration:
596 passed, ten top-level failures, 41 ignored. The nested crash helper is recorded
separately and is not an eleventh top-level failure. Exact names are retained in
workspace-failures01. Independent inventory/settings/CLI/configuration/vendor
runtime stages all pass. No full-workspace green claim; unrelated Files and prior
handoff/early-TLS issues remain open.

Retained negative evidence: installed-library legacy opaque/default-algorithm/
DigestOptions bugs; worker02 source-guard invalidation when verifier was edited
(reconstructed original verifier checked, stable worker03 passes); raw01 inherited
memfd EOF fixture error (pread0 correction, raw02 passes); qop control17 incorrect
expected outcome (actual listing succeeded, corrected controls18/19 pass). No red
receipt was rewritten as passing. All relevant processes are terminal.

## Evidence and next work

Sealed index: [authentication-runtime-evidence.json](print/authentication-runtime-evidence.json)
and target/portal-print-61/acceptance-01.json. Replay sources only are retained in
target/portal-print-61/replay. Phase net growth was about 43 MB, with 6.57 GB free;
no new Rust dependency graph, native fixture executable or cache cleanup.

Next implement [runtime selection and capability-bound credentials](print/runtime-integration-next.md).
The candidate still lacks a capability export. Keep CUPS out of the main/dialog
process and bind approved limits into request/prepared/cache authority. The raw
proof also exposed that system cupsSetUser truncates above 64 bytes although Rust
currently accepts 127: reject that before transmission under the legacy policy.
Actual Rust long-credential/private-channel, Iced/frontend both appearances,
metadata/Create-only/Send-only/recovery/held, identity/lost-ACK/resource/timeout,
device/Flatpak/runtime/package/release gates remain. Print stays opt-in.
