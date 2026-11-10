# Maintained owned IPP receiver

Status: canonical source/build provenance, loader selection and scoped native
acceptance verified. Print remains opt-in. The workspace retains the same ten
Files integration failures; physical-device and deployment acceptance remain open.

The stock IPP receiver rejected valid custom-media dimensions and advertised
duplicate MIME attributes when given an explicit attribute file. A diagnostic
correction enabled the stock-filter Canopa checks in
[ADR0107](0107-exact-paper-and-print-appearances.md), but its source/build pairing
was not yet a maintained fixture. Retain those original failures and their scope.

The separate `cups-ipp-receiver` overlay now pins the official CUPS 2.4.20 archive
in root provenance. Its one-file patch uses inclusive membership for both custom
dimensions, retains explicitly configured MIME attributes without duplicates,
and derives format capabilities from that same declaration. The original license,
CUPS exceptions and NOTICE are shared canonical assets. The complete 1,301-file
materialization matches its declared patch scope. No application-local fork,
runtime client source change or installed receiver is introduced.

The receiver includes private CUPS headers. Its builder therefore requires a
successful current client build receipt, verifies both source inventories and
binds `config.h`, `Makedefs`, `config.status` and the actual client image. The
client builder records these configuration inputs; old receipts reconfigure once,
and subsequent reuse rejects drift. That refresh produced exactly the existing
client903 bytes. Missing configuration bindings, a changed configuration hash
and a different library are refused before a receiver workspace is created.

The first maintained executable still selected system CUPS without its explicit
launcher environment. A measured loader check caught that gap. The final builder
retains the matching library once by checksum and emits a relative ELF RUNPATH to
that store. Both loader inspection and an actual eleven-case receiver run without
`LD_LIBRARY_PATH` select client903. System executables retain their system
libraries. The builder compiles one development C program, installs nothing,
retains a 3 GB reserve, bounds net growth to 256 MB and publishes the executable
once by checksum. A same-cache rebuild produces identical final bytes.

The maintained regression sends only validation/read-only IPP operations. The
unmodified, checksum-restored receiver fails seven of eleven cases: valid
interior/minimum/maximum/fractional media, nonfidelity handling, the outside-both
case and strict capabilities. The corrected receiver passes all eleven, including
exact declared PDF formats and no jobs. The restored baseline is retired again
after verifying its existing lossless archive.

The maintained receiver also passes actual Canopa worker/copy-only conversion,
stock configured CUPS filters and received stamped two-copy PDF checks, with all
eight before/after page rasters identical. Standard frontend, normal backend and
Iced consent pass dark/100% and light/125%, each with one-copy simplex and two-copy
long-edge delivery. Exact paper dimensions, printable margins, sides, page order,
labels and printed stamps are observed at the receiver. Nondefault logical
settings do not re-layout the prepared stream. The final loader-bound receiver
has its own public/native results; earlier prototype and intermediate receipts
retain their original binary identities.

The required original workspace/default/all-target incremental gate ran once.
Clippy and test compilation pass, followed by Portal439/55. The sole failed Cargo
target is Files integration with exactly the same ten failures as the earlier
gate; Settings, CLI, configuration and vendor poststages pass. Helper changes
after the shared preflight are confined to receiver tooling and have direct
compile/loader/native validation. No new Rust feature/profile graph or cache purge
was used. Older receiver executables are losslessly archived after checksum
verification; current binaries remain in the shared store.

See [the recipe](../vendor-maintenance.md#cups-client-ipp-backend-and-receiver),
[evidence](../../.scratch/xdg-portals/print/receiver-maintenance-evidence.json) and
[continuation checkpoint](../../.scratch/xdg-portals/PHASE71-LIVE.md).

Update, 2026-11-10: the `cups-ipp-receiver` overlay recipe was removed with `vendor/`. The receiver correction is now the `canopa/ippeveprinter-media-range` and `canopa/ippeveprinter-document-formats` topics of [kevinard/cups](https://github.com/kevinard/cups), with a tracking PR in the fork; the client, IPP backend and receiver are all built from its pinned `canopa/main` commit (`packaging/forks.json`). Bundles carry the original LICENSE/NOTICE and a summary naming the fork commit instead of patch files. See ../vendor-maintenance.md#cups-client-ipp-backend-and-receiver.
