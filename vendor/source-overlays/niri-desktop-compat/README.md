# Combined Niri desktop companion

Scoped native acceptance passes; full matrix/release acceptance remains pending.
Not installed or selected for distribution.

This recipe pins Niri `b22eb842ca9506d4e2ff997b116031cdb3f650f8` and its original
Smithay dependency `8a270232f1bfc416c781273c98a3f52a812a0bba`. It combines the
existing Niri static-frame resume correction with a port of the six-file Smithay
drag-source lifetime correction. The independent capture-only and Niri26.04
source-cancellation recipes remain unchanged as reproducible baselines.

The port uses the newer Smithay `InputTime::now()` interface. A dead active source
retires its pointer/touch grab immediately; a later input or Drop also checks
liveness. Destroying an older accepted source must leave a newer live grab alone.
Native pointer evidence must not be described as physical touch acceptance.

Exact archives, licences, patch scopes and hashes are in `vendor/provenance.json`.
Niri retains GPL-3.0-or-later, and Smithay retains MIT. The Niri patch and licence
are shared with the capture-only recipe; this directory retains Smithay's licence
and the patch against the newer pin.

```sh
python3 scripts/build-niri-screencast.py --overlay niri-desktop-compat \
  --archives /path/to/pinned/archives --work-dir target/niri-desktop-sources
```

`--prepare-only` verifies both patches and the two local source bindings without
building. `--toolchain NAME` selects the compiler explicitly. The helper preserves
every other locked dependency, builds locked/offline with one incremental dev job,
and reuses `target/niri-screencast-native`. Preserve prior executable snapshots
before rebuilding. It neither installs a compositor nor changes portal routing.

See [ADR0045](../../../docs/adr/0045-combined-niri-desktop-companion.md).
