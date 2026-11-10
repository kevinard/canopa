# Matching IPP backend preserves explicit resolution

Status: scoped worker/public-Iced/package acceptance; existing workspace Files failures retained.
Print remains opt-in.

The custom-margin continuation established that approved resolution survived
Canopa's worker and configured CUPS job, then disappeared in the stock IPP
backend's PPD conversion. An unmodified compiled backend reproduces that loss
with the same configured queue and stock `pdftopdf`. Its matching client, system
daemon/filter clients, source/type/quality, dimensions and margins are observed
independently. The failed baseline remains evidence.

A separate source overlay changes only `backend/ipp.c` from the pinned official
CUPS 2.4.20 archive. After PPD conversion it encodes explicit `printer-resolution`
with CUPS's standard job-option encoder, unless the attribute already exists.
Both axes and original units survive. No option means no added resolution.
There is no interposition, fabricated capability, default substitution, direct
device submission from Canopa or replacement of a configured driver. The earlier
interposition probe is not shipped.

The backend is compiled with the exact configured private client and its private
headers. The builder verifies both complete source inventories and configuration/
image bindings. The worker client stays unchanged. The daemon and stock filters
keep the system client; actual mappings confirm that separation. One backend is
retained by checksum and linked for both that development store and a matching
sibling client in a relocated package.

A discovery-only isolation check found that the first loader-only candidate
could start with the stock client when its matching client was unavailable.
No device URI or job was used. The final program requires Canopa client
capability v1 before any private API, with a strong imported symbol preventing
stock fallback. The packager verifies that dependency in the actual ELF. The
same missing-client control now fails before discovery; the matched relocated
pair still passes five owned IPP cases. This check supplements the exact
configuration/image binding performed during building and packaging.

The backend-entry regression passes 300 DPI, 600 DPI, 600 × 1200 DPI,
118 × 236 DPCM and the absent-option control. DPCM uses the backend ABI directly
and does not prove portal or legacy-queue exposure. The final packaged program
also passes three normal-worker submissions through owned configured CUPS queues
and stock filtering. They cover both paper sources, all three paper types and
quality levels, simplex and both duplex edges, all four unequal margins, exact
custom geometry, labels, printed stamps and identical rendered pixels. All jobs
complete at the owned receiver.

The standard frontend and normal Rust/Iced backend pass dark/100% and light/125%
consent flows. Each approves supported initial 600 × 1200 DPI hints, source/type/
quality and explicit custom-size rechecking, then sends one-copy simplex and
two-copy long-edge prepared PDFs through the packaged program and stock filter.
Device jobs retain approved typed resolution and controls. Exact physical boxes,
four/eight-side labels and printed stamps survive; logical range/reverse/scale/
N-up hints are not applied twice. The normal backend retains a sealed client
without mapping it; actual workers map the matching image.

Optional release packaging adds the program beside its matching client, a
relocatable checksum manifest, original LICENSE/NOTICE and complete source patch.
Staging requires the verified client/configuration and backend build. Checks
refuse another client, modified payloads/notices and symlinked files. A relocated
prefix passes actual loader and five owned IPP cases. Release scripts expose
`--cups-ipp-backend` with `--cups-runtime`; the release workflow includes and checks
both explicitly. No local release graph or full application archive is built for
this acceptance.

Execution remains the configured CUPS service's responsibility. Including this
artifact changes no backend directory, queue, device URI or desktop route.
Installed-service selection and real physical printing remain unverified. A PDF
job attribute does not prove physical or raster resolution. The stock PDF-only
driverless PPD generator still reduces supported resolutions to its default;
that capability work and general multiple-unit portal acceptance remain open.
Automatic Qt integration stays deferred.

Recipes and operator boundaries are documented in
[the overlay](../vendor-maintenance.md#cups-client-ipp-backend-and-receiver) and
[runtime packaging](../print-runtime.md). Baselines, native receipts and source-only
replays are indexed in
[the backend ledger](../../.scratch/xdg-portals/print/ipp-backend-evidence.json).
The final shared gate passes strict Clippy and Portal's 439 library/55 integration
tests. Its only failing Cargo target retains the same ten Files integration
failures. Settings, CLI, configuration and vendor post-checks pass; the full
workspace gate is not green.

Update, 2026-11-10: the `cups-ipp-backend` overlay recipe was removed with `vendor/`. The resolution correction is now the `canopa/ipp-backend-resolution` and `canopa/ipp-backend-capability-guard` topics of [kevinard/cups](https://github.com/kevinard/cups), with a tracking PR in the fork; the client, IPP backend and receiver are all built from its pinned `canopa/main` commit (`packaging/forks.json`). Bundles carry the original LICENSE/NOTICE and a summary naming the fork commit instead of patch files. See ../vendor-maintenance.md#cups-client-ipp-backend-and-receiver.
