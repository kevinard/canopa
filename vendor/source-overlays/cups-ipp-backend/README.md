# Optional matching CUPS IPP backend

This overlay pins the official OpenPrinting CUPS 2.4.20 archive and changes only
`backend/ipp.c`. The original Apache-2.0 LICENSE with exceptions and NOTICE are
shared with the private-client overlay through root `vendor/provenance.json`.

The PPD conversion path drops an explicit `printer-resolution` option. The
correction uses CUPS's standard typed encoder after that conversion, preserving
both axes and input units. It avoids duplicates and adds no resolution when the
option is absent. It does not synthesize a resolution from defaults or quality,
change device capabilities or alter PDF content.

Startup requires Canopa client capability v1 before any private CUPS API use.
The strong imported symbol refuses stock-client fallback if the matching client
is missing. Package checks require that dependency in the actual ELF; build and
package hashes bind the exact supported configuration and client image.

The program includes private CUPS headers. Build it against the verified exact
configured private client, never by loading that client into the stock backend.
The builder verifies both complete 1301-file source inventories, client recipe
and image, and `config.h`, `Makedefs`, `config.status` before and after building.
It compiles `ipp.c` and four unmodified support units in one compiler invocation,
bounds time/growth and retains a 3 GB reserve. It installs nothing, builds no
daemon/filter or Cargo graph, and publishes one backend by checksum.

From the workspace root, after building the client:

```sh
python3 scripts/build-print-ipp-backend.py --archives ARCHIVES --client-build target/cups-authentication-compat/canopa-build.json --work-dir target/print-ipp-backend-compat/maintained
python3 scripts/build-print-ipp-backend.py --archives ARCHIVES --client-build target/cups-authentication-compat/canopa-build.json --work-dir target/print-ipp-backend-compat/maintained --reuse
```

Keep that source path for later checks. The loader uses a matching sibling
`libcups.so.2` in a staged prefix or the matching checksum store in development.
System CUPS executables retain their own client library.

`scripts/test-print-ipp-backend.py --backend PROGRAM --library MATCHING_CLIENT --receiver OWNED_RECEIVER --out NEW_DIRECTORY`
checks three DPI choices, unequal DPCM axes and the absent-option control through
the actual backend entry point. It uses a private loopback receiver and no CUPS
daemon/queue. DPCM is backend-input acceptance, not evidence that a legacy queue
or portal exposes that choice. Full Canopa/configured-queue/stock-filter baseline
and candidate checks remain separate.

The optional release component requires the matching packaged client:

```sh
python3 scripts/package-print-runtime.py PREFIX --build-receipt target/cups-authentication-compat/canopa-build.json
python3 scripts/package-print-backend.py PREFIX --build-receipt target/print-ipp-backend-compat/maintained/canopa-build.json
python3 scripts/package-print-backend.py PREFIX --check --required
python3 scripts/test-print-backend-package.py --build-receipt target/print-ipp-backend-compat/maintained/canopa-build.json --client-build target/cups-authentication-compat/canopa-build.json --out NEW_DIRECTORY
```

The packager preserves the patch and original notices, verifies source/configuration
and image bindings, and stages `lib/canopa/print/ipp` beside its matching client.
Its manifest contains no build-machine paths. Checks refuse changed files, partial
inventory, another client or symlinked payloads. The regression relocates the
prefix, verifies actual sibling-library loading and can run five loopback cases
with `--receiver PROGRAM`.

Only an explicitly configured CUPS service executes this optional backend.
Packaging changes no service, backend directory, queue URI or desktop route.
Print stays opt-in. System deployment, physical/raster resolution and PDF-only
driverless capability reduction remain separate gates. See
[the decision](../../../docs/adr/0110-matching-ipp-backend-resolution.md).
