# Build profiles and free space

Run from the workspace root. Python 3 is required for the build guard:

```sh
bash scripts/cargo.sh build --locked -p canopa-shell
bash scripts/check.sh --files --test transfers
```

The normal `dev` profile uses `debug = "line-tables-only"` and incremental
compilation. Cargo's `test` profile inherits it. Function names and source
file/line information remain available for `RUST_BACKTRACE=1` panic backtraces,
including dependencies. Type and local-variable debugger information is omitted.
The [Cargo profile reference](https://doc.rust-lang.org/cargo/reference/profiles.html#debug)
describes that distinction. No dependency-wide `debug = false` override is used.

For a debugging session requiring types and local variables, opt in:

```sh
bash scripts/cargo.sh build --locked --profile debugging -p canopa-shell
bash scripts/cargo.sh test --locked --profile debugging -p canopa-config --lib
```

This profile inherits `dev`, enables full debug information and retains incremental
compilation. It writes to `target/debugging/`; normal dev/test outputs remain in
`target/debug/`. Use the same profile and features between ordinary edits to
reuse artifacts. The release profile is unchanged. Explicit Cargo environment or
configuration overrides still take precedence over the manifest settings.

## Disk guard

`scripts/cargo.sh` preserves Cargo's arguments, current directory, environment and
exit status. It runs Cargo through `scripts/build-guard.py`, which checks available
space before starting and once per second while running. If a reserve is crossed,
or a filesystem cannot be checked, it stops the command's process group, waits up
to five seconds, then kills surviving group members. A space failure returns 75;
interrupts return the usual `128 + signal` status. It never removes build outputs
or cleans caches. The guard is sampled protection, not a filesystem quota: very
fast allocation or unrelated processes can still exhaust a filesystem between
checks or during shutdown.

The default reserves are:

- **10 GiB** on filesystems containing the checkout, `target/`, the Cargo cache,
  environment-selected Cargo target/build directories and `--target-dir` outputs.
- **1 GiB** on the temporary filesystem selected by Python (`TMPDIR`, `TEMP`,
  `TMP`, then the platform default). A shared filesystem uses the larger reserve.

Missing output directories are checked through their nearest existing parent;
symlinks are resolved. Configurations placing outputs elsewhere through Cargo
configuration files or custom tools should supply those paths explicitly:

```sh
python3 scripts/build-guard.py --path /other/build-cache -- cargo build --locked
```

Both reserves accept positive fractional GiB values through
`CANOPA_BUILD_MIN_FREE_GIB` and `CANOPA_BUILD_MIN_TMP_FREE_GIB`, or the guard's
`--min-free-gib` and `--min-tmp-free-gib` options. Invalid or zero reserves are
rejected. For example, a small disposable fixture on tmpfs can explicitly choose
a smaller build reserve; normal desktop development should keep the defaults.

The check scripts, updater, native-fixture compilation helpers and build workflows
use the guard automatically. For manual compilation use `bash scripts/cargo.sh`
as above. A direct `cargo build` still receives the manifest's smaller debug
information, but bypasses the free-space guard. Read-only Cargo queries need no
wrapper. The standalone vendor-test workspaces also adopt the root debug setting
instead of silently restoring full debug information.

Changing profiles can leave older artifacts behind. No cleanup is required to
adopt these settings, and no automatic cleanup is performed.
