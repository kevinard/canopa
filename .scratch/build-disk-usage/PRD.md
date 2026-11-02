# Bounded build disk usage

Status: complete

Keep normal debug/test builds incremental in the root target directory, retaining
useful source file/line backtraces with smaller debug information. Provide an
explicit full-debug profile. Add a permanent preflight and running free-space
guard to maintained build/check/update entry points. Never delete caches or
artifacts automatically.

Validate low-space refusal, cancellation of a running command and its children,
signals/exit codes, check-script selection and updater behavior. Verify Cargo's
actual compiler flags, useful backtraces and incremental reuse with a small build;
avoid rebuilding the full desktop solely to test tooling.

## Implementation and validation

- Root dev/test builds now use line-table debug information with incremental
  compilation. The opt-in `debugging` profile enables full debug information in
  `target/debugging/`, retaining incremental compilation. Release is unchanged.
- `scripts/cargo.sh` runs commands through a permanent free-space guard. It
  checks build/cache filesystems against a 10 GiB reserve and a separate temporary
  filesystem against 1 GiB, before startup and once per second. Cancellation owns
  the command process group with bounded termination/escalation. It never deletes
  caches. Check/update/native build scripts and build workflows use the wrapper.
- Standalone vendor-test workspaces retain the root's smaller debug setting.
- Guard tests: 11 passed, including low-space admission, runtime refusal with a
  stubborn child, signals, exit codes, custom paths and temporary filesystems.
- Check-script selection/settings tests: 8 passed. Updater tests: 21 passed,
  including low-space preservation of the installed binaries/services/state;
  two affected scenarios passed again after routing fixture builds through the
  same guard.
- Actual canopa-config integration targets: intake 8 passed, themes 4 passed.
  Standalone vendor-pipe tests: 6 passed. The initial config library command also
  compiled successfully but contains no unit tests.
- A temporary example in the actual workspace verified panic function/source-line
  backtraces, the line-table compiler flag, unchanged executable reuse, edited
  incremental-directory reuse and the full-debug profile's incremental flag. The
  example source was removed afterward. The intake executable measured 14,396,808
  bytes with line tables versus 24,971,792 bytes with full debug information
  (42.3% smaller); this is one test target, not a workspace-wide savings claim.
- Cargo formatting, changed Python/shell syntax, TOML parsing and diff whitespace
  checks pass. Release documentation staging/check passes: 48 files and 69 links.
  The first staging command used an unsupported --stage option; corrected to the
  documented positional bundle argument and both checks passed.
- No full workspace/native acceptance or GUI rebuild was run for this tooling
  change. No existing root build cache was cleaned. The small verification builds
  reused root target/; existing independent updater fixtures remain disposable.

Execution logs, the profile probe and machine-readable profile evidence are local
under `.scratch/artifacts/build-disk-usage/`. Concurrent edits to `.gitignore` and
`docs/agents/issue-tracker.md` were preserved; their new artifact-location policy
was adopted for this work's generated output.
