# Full archive reproduction of the pointer companion

Follow-up (2026-11-01): the proposal below is retained as historical evidence.
The stronger `niri-desktop-pointer` recipe, including capture-creator cleanup,
is now registered and built from fully matched source inputs. Production Rust
admission and scoped media/input regressions pass on its exact binary; full
workspace remains red on ten Files tests. See
[the capture-owner ledger](../capture-owner/README.md). The older proposal digest
below is not the current canonical recipe.

Status: source reproduction passes; candidate remains unadopted and unbuilt,
2026-10-31. This closes the earlier changed-files-only replay gap and does not
establish new binary, native, physical-input or distribution acceptance.

The canonical keyboard/capture/drag recipe is materialized from its two pinned
upstream archives. Six retained pointer ownership/lease/source-metadata/mapping/
scale patches are applied with zero fuzz. Every file is compared against the
prepared source used by the existing scale-v2 pointer stack: 5,616 Niri files and
345 Smithay files match. Only the two absolute local Smithay dependency paths in
Niri Cargo.toml are normalized between work locations. Cargo.lock and all other
bytes match; no source mutation or compilation is needed for this comparison.

A draft combined recipe replaces overlapping historical layers with one patch
per source against the original upstream archive. It declares 22 changed/added
Niri files and eight Smithay files. The original GPL-3.0-or-later and MIT licenses,
source revisions, URLs and archive hashes are retained. Both combined patches
independently materialize from the raw archives through the existing strict vendor
verifier. The complete resulting trees match the layered materialization.
Manifest/lock transformations still change only the two pinned Smithay bindings;
all other dependencies remain unchanged.

[receipt.json](receipt.json) and [candidate-record.json](candidate-record.json)
record the source/archive/patch pins. The two patches, original license texts,
full file inventories and reproduction script are retained here. Working source
copies live under /tmp/canopa-pointer-archive-14/. The current canonical recipes,
accepted source trees, installed compositor and frozen executables are unchanged.
The workspace validation running during this check uses unchanged compiler inputs.

The recipe digest is
4c4e2db7a0ab4438acf49117bf6dd62c7ce9228ec17d7613a00f9f6936d2d29a.
This proposal is not registered in root provenance or selected by a build helper.
It must be reviewed and adopted with provenance, licenses and helper support,
then built and accepted on its exact executable before distribution. Existing
source-loss/physical/touch, broad shared/native reds and normal advertisement
remain open. See the [full roadmap](../ROADMAP.md) and
[atomic scale ledger](../ei-scale/README.md).
