# Optional Print client runtime

Canopa can bundle its modified CUPS 2.4.20 client beside the portal executable.
It supports the bounded credentials and Digest behavior described in the
[Print guide](print-portal.md). Print remains opt-in through `--experimental-print`;
shipping the client does not enable the interface or change the system scheduler.

Build the client with the pinned archive and the
[source recipe](../vendor/source-overlays/cups-authentication/README.md). Reuse the
same source/object directory for subsequent builds. Then pass its successful build
receipt to the ordinary release packager:

```sh
bash scripts/package-release.sh target/release dist --cups-runtime target/cups-authentication-compat/canopa-build.json
bash scripts/smoke-release.sh dist/canopa-x86_64-unknown-linux-gnu.tar.gz --require-print-runtime
```

The release workflow builds and includes this client explicitly. Invoking the
packager with only its original two arguments produces a bundle using system CUPS.
The packaging tools require Python 3 and GNU `readelf`; client compilation also
requires a C compiler, Make, pkg-config and GnuTLS development files.

The packaged layout is relative to one installation prefix:

| Path | Contents |
| --- | --- |
| `bin/xdg-desktop-portal-canopa` | Rust/Iced portal backend |
| `lib/canopa/print/libcups.so.2` | Private modified CUPS client |
| `share/canopa/print-runtime.json` | Upstream source, recipe, image and notice hashes; required shared-library sonames |
| `share/licenses/canopa-print-cups/` | Original upstream `LICENSE` and `NOTICE`, modification summary and complete source patches |

Keep these paths together when moving or installing a bundle. The backend finds
the library relative to its own executable and freezes a sealed image until exit.
Only isolated CUPS workers load it. The installed scheduler, filters and tools keep
their system library. `CANOPA_PRINT_CUPS_LIBRARY` is an explicit operator override;
ordinary use of a bundled runtime needs no environment override.

The client still needs the system libraries listed in `print-runtime.json`,
including GnuTLS. Those dependencies vary with the build host. They retain their
distribution notices and are not copied into the archive. The packager checks the
ELF architecture, soname, capability symbol and absence of embedded search paths
without loading the library. It verifies the canonical source patches, source
inventory, build receipt and exact library bytes before staging. The isolated
worker also checks the capability version when it loads the image.

The smoke check verifies the extracted library, manifest, patches and original
notices. A partial or altered bundle fails that check. Without a bundled client,
the backend uses conservative system-CUPS limits. It refuses an invalid present
library. The backend does not read the packaging manifest or notices; removing
those metadata files does not change its runtime selection.

## Optional configured IPP backend

The optional matching IPP backend preserves an explicit approved resolution that
the stock CUPS PPD conversion drops. It is compiled against the same exact private
client and headers. The system daemon and stock filters keep their own libraries.

```sh
python3 scripts/build-print-ipp-backend.py --archives ARCHIVES --client-build target/cups-authentication-compat/canopa-build.json --work-dir target/print-ipp-backend-compat/maintained
bash scripts/package-release.sh BUILD_DIR OUTPUT_DIR --cups-runtime target/cups-authentication-compat/canopa-build.json --cups-ipp-backend target/print-ipp-backend-compat/maintained/canopa-build.json
bash scripts/smoke-release.sh ARCHIVE --require-print-runtime --require-print-backend
```

The program is `lib/canopa/print/ipp`, beside its required matching `libcups.so.2`.
`share/canopa/print-backend.json` binds the program, client and canonical recipe;
`share/licenses/canopa-print-ipp-backend/` retains original LICENSE/NOTICE and
`resolution.patch`. Keep the pair together when relocating. Staging refuses
source/configuration drift, another client or an unverified build. Bundle checks
refuse partial or altered inventories and symlinked payloads. The release workflow
includes both components explicitly; ordinary two-argument packaging is unchanged.

Including this artifact does not select it for CUPS. Execution depends on an
explicitly configured CUPS service's backend directory; Canopa continues to submit
to its selected queue and does not override the device URI. No installed service,
backend directory or queue is changed by the build or packaging helpers. Installed
service selection and physical-device acceptance remain open.

The backend requires Canopa client capability before using private CUPS APIs.
If its matching client is unavailable, a strong symbol dependency prevents stock
CUPS fallback. Package verification checks that dependency and the exact paired
image hashes; a discovery-only missing-client regression fails before a device
connection or private API use.

The packaging test relocates a small component prefix, verifies actual matching
sibling-client loading, and can run five owned IPP cases:

```sh
python3 scripts/test-print-backend-package.py --build-receipt target/print-ipp-backend-compat/maintained/canopa-build.json --client-build target/cups-authentication-compat/canopa-build.json --out NEW_DIRECTORY --receiver OWNED_RECEIVER
```

This does not prove physical/raster resolution or expose additional resolutions
that the configured queue omits. See [the backend decision](adr/0110-matching-ipp-backend-resolution.md).

For a focused packaging regression check after building the optional client:

```sh
python3 scripts/test-print-runtime-package.py --build-receipt target/cups-authentication-compat/canopa-build.json --binary target/debug/xdg-desktop-portal-canopa
```

That check uses disposable staging and metadata-only Rust workers. It does not
establish a physical printer, a deployed desktop or complete Print acceptance.
