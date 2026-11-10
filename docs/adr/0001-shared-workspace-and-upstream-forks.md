# Shared workspace and upstream dependency forks

Status: Accepted for the first integration step, 2026-09-26.

Canopa now preserves Niribar and Notify in one Git repository and one Cargo workspace.
The import commit has both previous heads as parents; `legacy/niribar` and
`legacy/notify` retain their original ancestry. Source directories, binaries,
configuration paths, service identities, and notification storage stay in place.

The root manifest owns dependency versions, release settings, and crates.io patches.
Applications inherit dependencies and add their own features. One workspace crate
owns the generic panel protocol. This supersedes Niribar ADR 0011's independent
vendored-snapshot arrangement, while preserving its explicit v1/v4 adapters and
notification-domain isolation. Notify's old nested workspace is removed.

Shared forks use the latest non-yanked stable releases verified in the official
registry on 2026-09-26: iced_layershell/layershellev 0.19.1 and accesskit_unix 0.24.0.
Keep only local behavior still absent upstream. Popup grabs/dismissal and
compositor-selected output placement now use upstream mechanisms. Output snapshots,
activation context and retired-window cleanup remain local. The AT-SPI cache signal
correction was local until accesskit_unix 0.24.1 released it upstream (#819); that
fork was dropped on 2026-11-10. Archive checksums, upstream commits, file hashes and patches are
recorded together; see ../vendor-maintenance.md.

The root release profile adopts Niribar's thin LTO, one codegen unit and symbol
stripping. Release builds unwind on panic, like the tested dev profile: Files, Notify
and the portal contain worker panics with `catch_unwind` and report a failed job,
reader or worker instead of losing the process. The original abort-on-panic policy
made that recovery unreachable in shipped builds only (2026-11 review). Core-only Notify remains a
separate package-selected build. Cargo's additive feature unification means a
combined GUI build also enables the shell's theme detection in the shared renderer;
Notify-only builds retain their previous disabled default. Validate core and combined
desktop profiles independently. Shared configuration and appearance remain later work.

No upstream pull request or public fork is published by this change. The tracked
`vendor/` trees and reviewable patches are the local forks.
