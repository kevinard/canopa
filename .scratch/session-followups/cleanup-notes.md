# Local validation artifact cleanup — 2026-11-02

Status: complete

Removed 2,379 loose generated files and 205 empty directories from the session,
greeter, polkit, pinentry and wallpaper validation runs. Approximately 41.5 MiB
of allocated space was recovered after accounting for the retained archive.
The cleanup did not run Cargo or alter its build caches. Separate build-tooling
work in progress was outside the cleanup scope.

## Preserved

- All tracked plans, notes and performance evidence; 42 tracked files in the
  affected note/evidence trees were checked against their pre-cleanup hashes.
- The Stasis investigation ticket, handwritten diagnostic scripts, historical
  execution runners and preview configurations in their original locations.
- The [release archive](release/canopa-x86_64-unknown-linux-gnu.tar.gz) and
  [checksum](release/canopa-x86_64-unknown-linux-gnu.tar.gz.sha256).
- 1,942 result files, logs, screenshots, event traces and other run evidence in
  [the local evidence archive](../artifacts/cleanup-2026-11-02/validation-evidence.tar.gz).
  Every member was verified against its original SHA-256 before removing the
  loose copy. Original paths and hashes are recorded in
  [the cleanup manifest](../artifacts/cleanup-2026-11-02/manifest.json).
- Selected benchmark evidence remains committed under
  [performance data](../../docs/performance/data/).

The archive SHA-256 is
`cb1b2d73aefa40e93cf97bc706c83fd8d7f42d257631d167d64d38e4560162fc`.

To inspect an archived result from the workspace root without recreating the
loose run directories:

```sh
tar -xOf .scratch/artifacts/cleanup-2026-11-02/validation-evidence.tar.gz \
    .scratch/session-followups/validation-summary.json
```

## Removed regenerable files

Discarded compiled keyboard/pointer/PAM/NSS helpers, the staged test compositor,
generated Wayland C/header bindings, shader/Python caches and duplicate staged
documentation bundles. The two leftover Files temporary-test files were archived
before their directory was removed.

## Ignore policy

Scratch Markdown plans and notes remain visible to Git. Other scratch output is
ignored by default; new run output should go under `.scratch/artifacts/<feature>/`,
which also ignores generated Markdown. Maintained tools belong under `scripts/`.
The existing prototype/archive exclusions remain in effect. Files' `.tmp*`
test directories are ignored too.

Validation: archive member/hash checks; preserved-file hash checks; 19 ignore-rule
checks covering generated output, legacy exclusions, notes, source and committed
performance data; whitespace checks on the housekeeping changes. No builds ran.
