# Target cleanup — 2026-10-29

Removed the extra investigation build directories at the user's request. Ordinary
`target/debug` and `target/release` outputs were left intact.

Deleted the rebuildable `target/uutils-proof` cache and the build intermediates
under `target/niri-cancel-upstream`. The proof script can recreate its cache when
explicitly run; normal development checks continue to reuse root `target/`.

Retained investigation artifacts now live under
`.scratch/file-manager/retained-target-artifacts/`, outside the Cargo output tree:

| Historical path | Current path |
| --- | --- |
| `target/copy-opt-evidence` | `.scratch/file-manager/retained-target-artifacts/copy-opt-evidence` |
| `target/uutils-preserved` | `.scratch/file-manager/retained-target-artifacts/uutils-preserved` |
| `target/uutils-release-debug` | `.scratch/file-manager/retained-target-artifacts/uutils-release-debug` |
| `target/niri-cancel-candidate-source-07` | `.scratch/file-manager/retained-target-artifacts/niri-cancel-candidate-source-07` |
| `target/niri-cancel-candidate-source-10` | `.scratch/file-manager/retained-target-artifacts/niri-cancel-candidate-source-10` |
| `target/niri-cancel-recipe-check-17` | `.scratch/file-manager/retained-target-artifacts/niri-cancel-recipe-check-17` |
| `target/niri-cancel-upstream/debug/niri` | `.scratch/file-manager/retained-target-artifacts/niri-cancel-upstream/debug/niri` |

Historical notes and receipts retain their original paths and hashes. Resolve
those paths using this table. `relocation.json` in the retained directory records
every retained file's SHA-256, size and permissions; all files were checked before
and after relocation.

For native acceptance, use the preserved executable explicitly:

```sh
python3 scripts/run-desktop-acceptance.py \
  --niri .scratch/file-manager/retained-target-artifacts/niri-cancel-upstream/debug/niri \
  --niri-sha256 63e39bae347346d7ac5aa90fb974ee8fe09ee59d9bd69238546e990368232da7
```

Validation: retained file inventories and hashes match, and the Niri executable
matches its existing build receipt. No build or test suite was run because this
cleanup changed only local outputs and their location note.

## Debug cache cleanup

Removed 6.44 GiB from `target/debug`, reducing
its allocated size from 18.91 GiB to
12.48 GiB. The removal set contains old Niribar
package artifacts, artifacts for a previous compiler, dependency versions absent
from the current lockfile, broken temporary-cache links and abandoned compiler
temporary directories. This cleanup does not use file age to evict dependencies.

Preserved every current compiler/dependency feature variant unless its registry
version is absent from the lockfile. Also preserved 53 candidate fingerprints
referenced by retained artifacts, including their transitive dependencies. The
entire incremental cache was excluded from removal.

Validation: the 31,829 retained files have unchanged
identities, sizes, modes and modification times; all
4,797 incremental entries match their original
inventory. Cargo build-directory locks were held during cleanup. No workspace
build or test suite was run. An empty disposable crate established the pinned
compiler fingerprint without changing the workspace build cache. Offline Cargo
metadata succeeded when filtered to the host platform; the initial unfiltered
attempt reported the uncached foreign-platform dependency `wild v2.2.1`.

The ignored local removal manifest is
`.scratch/file-manager/retained-target-artifacts/debug-cleanup-2026-10-29.json`.
