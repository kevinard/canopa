# Portal monitor identity integration

Status: Rust/Iced portal path implemented with scoped native acceptance,
2026-10-31. The compositor extension remains an unadopted draft. No pointer
permission, normal RemoteDesktop registration or descriptor advertisement is
introduced here.

The portal reads live monitor identities before showing consent and carries them
through chooser refresh, selection, revalidation, worker serialization and
RecordMonitor. A replacement identity clears selection even if the connector and
title are unchanged. Review refresh also requires the original unique compositor
owner. The worker requires the identity precondition and checks the returned
Parameters identity after the producer starts, before reporting ready. Geometry
is read at that same point. Failed or ignored identity checks stop the session.

Monitor values retain legacy string serialization when the compositor has no
extension; this preserves existing view-only capture. Pinned values use a strict
structured representation. Query failures and unsupported/malformed extensions
cannot silently downgrade a pinned selection. Duplicate connector aliases are
rejected even when their identity fields differ. Existing unique window IDs also
reach RecordWindow as the optional precondition.

| Final scoped check | Result |
| --- | --- |
| Portal library | 145 passed, 4 ignored |
| Complete portal integration executable | 37 passed, 2 ignored |
| Portal all-target Clippy with warnings denied | Passed |
| Identity-capable compositor, dark/125%, real video and Iced controls | 17 groups; 9 of 9 monitor calls carry identity |
| Existing canonical compositor, light/100%, legacy capture | 17 groups; 9 monitor calls retain legacy semantics |
| Identity-capable compositor, light/125%, combined keyboard EIS/media lifetime | 6 groups; 6 of 6 monitor calls carry identity |

The normal incremental debug cache/default features are retained. Final focused
validation takes 75.39 seconds; the three native suites take 254.31 seconds
including startup and artifact preparation. All 1,303 workspace/vendor source
inputs and recorded fixture/binary pins stay unchanged. All owned jobs and
process groups terminate. Evidence is at target/portal-source-identities-06/;
[evidence.json](evidence.json) identifies exact executable, patch and source hashes.
[portal.patch](portal.patch)
is the ten-file delta against this slice's preserved starting state. Native
sources are retained here as well as beside their execution receipts.

Earlier attempts remain evidence: a moved-name compile error; two incorrect
integration filters that selected zero tests and do not count as coverage; and
two Clippy findings. One later normal parallel library run passes 143 tests but
times out in existing Wayland-handshake and input-worker cancellation tests. Its
cause remains unresolved. Both cases pass directly on the identical executable
in 0.07/0.34 seconds, and the full normal parallel rerun passes 145 tests. This is
not an explanation or blanket classification of those failures as flaky.

The source identity extension must be adopted and distributed before relying on
it for pointer authorization. The subsequent [private pointer worker
slice](../pointer-workers/README.md) integrates capture-bound ownership and lease
renewal with scoped native acceptance. EI/Notify pointer adapters, explicit Iced
pointer approval and remaining window behavior still need integration.
Earlier full workspace/native matrix failures and
physical input/distribution acceptance remain open. The full 18-interface goal
is unchanged. See [ADR0048](../../../docs/adr/0048-live-capture-source-consent.md)
and [the capture mapping ledger](../POINTER-MAPPING.md).
