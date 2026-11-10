# Package the private Print runtime and its notices

Status: scoped local packaging and native discovery verified; release/deployment
and full Print gates remain. Print stays opt-in.

The backend can discover `PREFIX/lib/canopa/print/libcups.so.2` beside its own
executable, but release packaging previously omitted that library. Operator-only
environment selection could not establish that an ordinary relocated bundle used
the verified client. Its upstream notices also needed a distribution destination.

Add an explicit `--cups-runtime BUILD_RECEIPT` release packaging option. The helper
requires a completed build matching the current canonical recipe, verifies the
source inventory and exact image bytes, and inspects ELF metadata without loading
the library. The archive receives the private client, original upstream LICENSE
and NOTICE, complete patch, modification summary and a relocatable checksum
manifest. The manifest records required system-library sonames and build flags;
it contains no local source paths. System libraries are not bundled.

The extracted-archive smoke check verifies those assets and rejects partial or
altered runtime contents. `--require-print-runtime` also refuses a bundle without
the optional client. The original two-argument packaging command retains system
CUPS behavior. The release workflow builds the pinned upstream archive and selects
the bundled client explicitly. No system installation or publication occurs here.

The packaging regression driver passes staged/missing/partial runtime checks,
stale and incomplete receipts, altered library/license/notice/patch checks, and
file/directory symlink refusal. Directory symlinks are rejected before staging can
write through them. Twelve actual Rust-worker metadata cases pass against the
staged image, including quoted realms and maximum UTF-8 credentials.

A full archive built from the eight existing debug executables passes all normal
release smoke checks and the required runtime check. This validates packaging,
not a newly compiled release profile. All five runtime assets match their archive
manifest. The duplicate 179 MB test archive is removed after its checksum and
receipts are retained; root executables remain the authoritative copies.

A separate relocated-prefix native run uses a hard link to the current backend,
avoiding an executable data copy. With both runtime-selection environment
variables absent, the actual backend and metadata UI retain the expected sealed
image, while only CUPS workers map it. Credential UI has no image descriptor.
Three public frontend/Iced consent/file-output groups pass with quoted SHA-256
fields and prepared four-page PDF delivery. All 30 recorded process identities
are terminal and private data is removed. This is a private desktop/scheduler
check, not deployment or physical-device acceptance.

See [the user guide](../print-runtime.md),
[scoped evidence](../../.scratch/xdg-portals/print/packaged-runtime-evidence.json)
and [remaining work](../../.scratch/xdg-portals/print/authentication-connection-next.md).
No Rust source, Cargo graph or canonical CUPS patch changes were needed. The
previous non-green workspace result remains recorded; tooling/documentation checks
cover this change. The Ubuntu release workflow is configured but has not run in
this local validation. Automatic Qt routing stays deferred, and the full
eighteen-interface goal remains active.

Update, 2026-11-10: the `cups-authentication` overlay recipe was removed with `vendor/`. The packaged client is now the pinned fork of [kevinard/cups](https://github.com/kevinard/cups), with a tracking PR in the fork; the client, IPP backend and receiver are all built from its pinned `canopa/main` commit (`packaging/forks.json`). Bundles carry the original LICENSE/NOTICE and a summary naming the fork commit instead of patch files. See ../vendor-maintenance.md#cups-client-ipp-backend-and-receiver.
