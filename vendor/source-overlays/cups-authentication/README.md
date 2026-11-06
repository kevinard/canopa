# CUPS authentication compatibility candidate

This optional source overlay builds a process-private CUPS client library for
Canopa's isolated printing workers. Runtime selection is implemented and under native acceptance;
the release packager can include the verified library explicitly. The installed CUPS daemon and tools keep
their system library.

The official OpenPrinting 2.4.20 release archive is pinned by SHA-256 in the root
provenance record. Its Apache-2.0 licence with exceptions and NOTICE are retained
unchanged. The eight-file patch corrects negotiated Digest qop and its default MD5
algorithm, echoes legacy opaque values, applies parsed DigestOptions, and makes
client username/credential/Base64/hash buffers large enough for Canopa's bounded
127-byte usernames and 1023-byte passwords. Oversized combined values and failed
header allocation are refused rather than sent after truncation.

Quoted challenge fields are decoded before hashing and before exposing the realm
to the private prompt. Digest output escapes quotes and backslashes in usernames,
realms, nonces, opaque values and request targets, while the hash uses the original
bytes. Quoted values are skipped as complete fields when finding authentication
schemes. Complete header size is checked before it is retained or sent.

An incomplete HTTP response remains a read error even when its status line says
401. Request setup stops before authentication or reconnect if the remaining
headers cannot be read. This prevents an EOF loop and avoids continuing from a
truncated challenge. A failed document write remains unconfirmed, including when
the peer sent a rejection that the client did not observe.

The separate one-file `custom-media.patch` assigns a custom range's declared
bottom margin to the bottom field. Upstream writes it to the top field and
leaves the bottom at a 12.7 mm fallback. The original eight authentication files
and their patch are unchanged. The metadata-only custom-paper regression checks
all four margins through the actual worker and configured CUPS driver; asymmetric
fixtures keep top and bottom distinguishable. This does not correct the separate
IPP backend resolution forwarding or PDF driverless capability limitations.

The patch changes private CUPS structures. The portal loads it only into a dedicated worker
that uses opaque public CUPS APIs; never inject it into an existing system daemon,
filter or module compiled against another private layout. The original prototype probes
selected it only in their Rust worker child and checked its actual mapping.
The scheduler and setup tools retain the system library. The integrated selector
instead retains one sealed anonymous image and passes its descriptor only to CUPS
workers and the metadata dialog; only standalone CUPS workers load it.

Build from the workspace root with downloaded archives:

```sh
python3 scripts/build-cups-authentication.py --archives ARCHIVES --work-dir target/cups-authentication-compat --prepare-only
python3 scripts/build-cups-authentication.py --archives ARCHIVES --work-dir target/cups-authentication-compat --reuse
```

The builder verifies all 1301 source files and exact patch scope, uses one compiler
job, retains the same sources and object cache, and builds only libcups.so.2 with
GnuTLS. It performs no installation. It retains a disk reserve and a build receipt.
Canonical LICENSE and NOTICE are both checked by the shared overlay verifier.

Build receipts also bind `config.h`, `Makedefs` and `config.status`. An older
receipt reconfigures once before compiling; subsequent reuse rejects configuration
drift. This permits the separate test-only IPP receiver to verify its header/client
pairing. `--min-free-bytes` explicitly selects a positive reserve for one build;
the default remains 6 GB. No runtime source or authentication capability changes
are implied by this receipt extension.

The maintained regression tool uses a strict independent byte-level oracle and a
private metadata-only HTTP/IPP endpoint. Its self-test checks published MD5 and
SHA-256 vectors, quoted-pair syntax and malformed input without native services;
the workspace gate runs it. Native runs require a built normal Portal binary and
an explicitly selected client image:

```sh
python3 scripts/test-cups-authentication.py --self-test
python3 scripts/test-print-early-replies.py --library target/cups-authentication-compat/cups-2.4.20/cups/libcups.so.2 --receipt target/cups-authentication-early-replies.json
python3 scripts/test-cups-authentication.py --binary target/debug/xdg-desktop-portal-canopa --library target/cups-authentication-compat/cups-2.4.20/cups/libcups.so.2 --artifacts target/cups-authentication-native
python3 scripts/test-cups-authentication.py --binary target/debug/xdg-desktop-portal-canopa --library target/cups-authentication-compat/cups-2.4.20/cups/libcups.so.2 --tls --artifacts target/cups-authentication-tls-native
```

Use a new artifacts directory for each invocation. The tool freezes one sealed
image, verifies its actual worker mappings, bounds helper lifetime and confirms
retirement. It tests maximum quoted ASCII/UTF-8 credentials and refusal controls,
including a fake Basic offer inside a quoted opaque value. It creates no jobs and
uses no actual scheduler, Iced dialog, PAM account or device. Real frontend/Iced
and scheduler acceptance remains separate. TLS mode also requires `openssl`.

The library exports `canopaCupsAuthenticationVersion() = 1`, declaring the exact
bounded credential and Digest fixes. The worker checks this extension before using
an explicitly selected image; a CUPS version string alone cannot enable larger
credentials. The system soname keeps conservative 64-byte usernames and 189-byte
combined values. Recognized version1 permits 127-byte usernames and 1023-byte
passwords, with 1151 bytes including the colon. Unknown/missing capabilities fail.
Typed private challenge metadata binds that choice into cache and prepared handoff.

The separate optional IPP backend is another supported consumer only when it is
compiled from pinned source against this exact configured client and private
headers. Its build/package checks bind that pairing. This does not permit loading
the client into an existing system backend; see
[the matching companion](../cups-ipp-backend/README.md).

For operator testing, set `CANOPA_PRINT_CUPS_LIBRARY` to the absolute regular file
path when starting the backend with `--experimental-print`. The optional installed
location is `PREFIX/lib/canopa/print/libcups.so.2`, relative to its executable in
`PREFIX/bin`. Missing bundle selects the system library; an invalid present bundle
or explicit selection stays unavailable. The backend freezes its image until exit,
so changing the source path cannot replace code beneath a prepared request.
This build recipe performs no packaging or installation. To include its verified
output in a release, use the packager's `--cups-runtime` option described in
[runtime packaging](../../../docs/print-runtime.md). The archive includes the
original LICENSE/NOTICE, a modification summary, complete patch and checksum
manifest. The release workflow selects this option; Print remains opt-in.

Runtime/capacity integration is under validation. Worker checks alone do not
establish Iced/frontend, protected recovery/held-job, resource, device, deployment
or complete Print acceptance. Print remains opt-in. See
[the runtime decision](../../../docs/adr/0101-process-private-print-authentication-runtime.md).
