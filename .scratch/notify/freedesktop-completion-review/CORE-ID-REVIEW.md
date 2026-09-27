# Core-ID continuation review

User-selected baseline: `df88ecda9f2170ff9a1d1e3f62531882a786761e`. Cumulative command: `git diff df88ecd`; committed portion: `git diff df88ecd...HEAD`, including `8be0e99` (revision-safe banners and coalesced presentation) and `9289e34` (protocol extraction and issued-ID retirement). Both reviewers also inspected the new, initially untracked `notification_id.rs` and `dbus_restart.rs`. The current W03 tranche began at `9289e34`.

The implementation skill required independent, parallel Standards and Spec reviews. Scope is the recorded continuation in [IMPLEMENTATION.md](IMPLEMENTATION.md), not completion of every package.

## Standards

No documented-standard violations or actionable heuristic smells were found in the cumulative diff against `df88ecd`, including the new files.

The shared `InternalNotificationId` follows CONTEXT.md's domain terminology and ADR 0029's boundary: raw wire replacements no longer enter core intake, while lifecycle, history, runtime, presentation, sound and control interfaces preserve typed identity. Numeric construction remains at storage/control adapters and runtime allocation.

Serialization retains signed integer representations; legacy zero replacement metadata is normalized without admitting zero thread identities. Checked runtime-only allocation cannot wrap into retained IDs. The intentional Rust API change is documented.

The restart fixture observes real service-owner transitions, separates wire IDs from retained identities, verifies stale history is unchanged and checks successor closure identity. Child cleanup covers assertion failures.

Cumulative banner revision handling and the synchronized presentation send/retirement gate remain consistent with ADR 0030. The registry's guarded preparation/commit and retirement remain intact after sharing the internal type. No additional correctness concern was found. Tooling-enforced checks were excluded from this axis.

## Spec

Zero scoped defects and no scope creep were found against `df88ecd`, including the new ID type and restart fixture.

W03's requirement to "pass a typed resolved target into Notification Lifecycle" is implemented throughout intake, receipt, lifecycle/history, runtime, sender events, and presentation/control interfaces. Content decoding no longer interprets wire IDs; live resolution remains inside the receipt mutation boundary.

The signed numeric JSON/SQLite representation is preserved. Rejecting zero as an actual identity and normalizing legacy zero replacement metadata match the documented compatibility contract. Checked runtime-only allocation cannot wrap into retained IDs.

Restart coverage exercises two actual daemon processes, owner disappearance/reappearance, stale retained history, transient-state loss, fresh registration, replacement, and closure identity. It correctly distinguishes permitted numeric recurrence across daemon lifetimes from accidental registration of old history.

The cumulative banner revision/coalescing changes retain the preceding correction for concurrent renderer retirement and publication.

W03's separate trustworthy-identity/evidence decision, broader failure/recovery verification, graphical acceptance, and other completion packages remain explicitly unfinished. They are not omissions from this scoped tranche.

Summary: Standards — 0 findings, no worst issue; Spec — 0 scoped findings, no worst issue. No review correction was required. Final verification evidence is recorded in [IMPLEMENTATION.md](IMPLEMENTATION.md).
