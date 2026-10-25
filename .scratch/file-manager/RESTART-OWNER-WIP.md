# Stopped-transfer restart execution owners

The full five-area goal remains active. This checkpoint supplies actual owners
for durably stopped Copy/Move. Running and Queued requests still need further
restart work.

## Verified final source

`restart-owner-inputs-04.json` captures 859 Rust/Cargo/vendor/fixture paths.
`restart-owner-verification-01.json` verifies that source, the additional schema
fixture, protected app/Niri binaries and the earlier local-runtime gate.

- `restart-owner-lib-05.log`: 83 passed, seven ignored.
- `restart-owner-core-15.log` through `restart-owner-core-23.log`: all nine batches
  passed on unchanged inputs. Together with the library, 381 parents pass across
  19 no-default-feature targets; 25 ignored entries are inventory helpers.
- The required unignored Copy/real cross-filesystem Move gate passes both part
  orders. `restart-owner-required-gate-01.json` records the exact gate executable.
- A second actual SIGKILL between parts preserves the unselected owner and refuses
  a second handoff of the consumed part. Successors remain independently undoable.
- Refusal cases cover replacement selections, actual parents with the same
  entries, destination roots/traversed output folders, changed plans/revisions,
  missing seals and copied history databases. Sealed unavailable parts can be
  abandoned; independently proved parts remain usable.
- A deterministic activity regression proves that restored accepted work blocks
  device removal until explicit abandonment. Its startup barrier itself creates
  no activity guard.
- `restart-owner-desktop-lib-01.log`: 164 passed, eight ignored, under private
  D-Bus/HOME/XDG directories. `continuation-core-1792639510-receipt.json` verifies
  all captured inputs before/after. No live desktop, audio server or devices used.
- `restart-owner-static-checks-01.json`: default all-target Clippy with warnings
  denied, formatting and schema 38 fixture syntax pass.

## Implementation and evidence

Schema 38 adds the original physical history file to new accepted transfer
intents. Exact stopped seals, current revision/body, immutable report and part
hashes are checked before reopening original source/destination objects. Original
parents, generations and traversed folders remain required. Reopening never
captures replacement originals. The raw history filename is also tested.

Handoff checks one exact successor checkpoint and advances the stopped seal in
the same transaction as acceptance. Abandon advances only abandoned dispositions.
Generic scheduling writes cannot mint this progression. Restored owners retain
the startup activity guard until their exact work is released.

The original missing-owner failure is in `restart-owner-red-01.log`; its private
roots remain intact and are decoded/verified in `restart-owner-red-fixture-01.tar.gz`.
The fixture verification JSON records its hash. The activity regression fails in
`restart-owner-activity-red-01.log` and `restart-owner-activity-red-02.log` before
the guard fix. Earlier core runs retain intermediate failures: old shutdown tests
needed to account for newly restored owners; the schema 29 compatibility fixture
now removes the later history proof to represent a genuine older intent.

The final source archive and decoded-member verification are
`restart-owner-source-01.tar.gz` and `restart-owner-source-01-verification.json`.
Read the actual verification file for its final hash and member count. Earlier
checkpoints and all failure evidence remain retained. The schema 27 application
and compatible Niri remain byte-for-byte unchanged. No commits were made.

## Remaining work

Active-execution and queued restart continuation, metadata-save retry ownership,
per-child source identity between publication and source removal, saved-container
claims, complete Preparing/Cleanup/migration binding, legacy handling and durable
successor lineage for queued recovery remain open. General Undo/relocation/expiry
must consume newer original facts, including directories recreated by earlier
Undo. Trash/Restore restart and all other four goal areas remain required.

The earlier intermittent paused-video/seek timeout is undiagnosed; a passing
isolated suite does not resolve it. No native, full workspace/release, Shell/Notify
regression or physical-device acceptance is claimed here.
