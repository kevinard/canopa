# Owned IPP receiver fixture

This test-only overlay pins the official OpenPrinting CUPS 2.4.20 archive and
patches only `tools/ippeveprinter.c`. The original Apache-2.0 license with
exceptions and NOTICE are shared with the CUPS authentication overlay and
referenced explicitly in root `vendor/provenance.json`.

The receiver checks both dimensions against inclusive advertised media ranges.
The original inverted predicate rejected valid interior sheets and could accept
an out-of-range dimension. Explicit attribute-file MIME declarations are retained
without duplicate attributes, and format-dependent capabilities use that same
declared list. No response-normalizing relay is used. The original implementation
and corrected diagnostic have separate retained red/green protocol evidence.

The receiver uses private CUPS headers, so it must be compiled against the exact
configured source and library from a verified client build. Never inject that
library into a system daemon, backend or filter built with other private headers.
The separate matching backend companion is compiled and verified independently.
Build from the workspace
root, reusing stable source paths:

The commands below assume an existing configured client cache. For a first build,
follow [the client recipe](../cups-authentication/README.md) first.

```sh
python3 scripts/build-cups-authentication.py --archives ARCHIVES --work-dir target/cups-authentication-compat --reuse
python3 scripts/build-print-ipp-receiver.py --archives ARCHIVES --client-build target/cups-authentication-compat/canopa-build.json --work-dir target/print-ipp-receiver-compat/maintained --prepare-only
python3 scripts/build-print-ipp-receiver.py --archives ARCHIVES --client-build target/cups-authentication-compat/canopa-build.json --work-dir target/print-ipp-receiver-compat/maintained --reuse
```

The client builder now binds `config.h`, `Makedefs` and `config.status` into its
receipt. An older receipt is reconfigured once before the unchanged library is
built; later reuse rejects configuration changes. Its default space reserve is
6 GB; an explicit `--min-free-bytes` selects a positive reserve for that invocation.
The receiver builder independently retains 3 GB and bounds net build growth to
256 MB. It checks both complete source inventories and the client's configuration,
recipe and library checksum before and after compilation. `--library PATH` can
select the same verified image at its retained checksum path.

Only one C program is compiled, with development flags and no installation. The
executable is published once into the shared checksum-addressed fixture store;
reuse never makes per-test binary copies. The build receipt records its exact
source, compiler, configuration, library and binary bindings.
The default ELF loader path is relative to that store and selects the matching
checksum-retained client without requiring `LD_LIBRARY_PATH`. Explicit diagnostic
launchers must also use that same client; system executables keep their libraries.

Use `scripts/test-print-ipp-receiver.py --binary PATH --library LIBRARY --out NEW_DIR`
for validation-only requests against a private loopback instance. It checks custom
range limits, one-dimension failures, nonfidelity behavior, strict capabilities
and the absence of jobs. Real Canopa/frontend/Iced/configured-filter delivery is
a separate acceptance layer. No installed queue, physical printer, service or
desktop route is changed by these helpers. Print remains opt-in.
