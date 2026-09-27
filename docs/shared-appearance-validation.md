# Shared appearance validation

Local integration validation, 2026-09-26, against the configuration baseline
`1682ef3`. Scope and ownership are recorded in [ADR 0003](adr/0003-shared-appearance.md).

The configuration tests exercise shared appearance when either feature namespace is
absent, original source offsets, and content-free rejection in both validators.
The notification presentation test loads a light snapshot, repaints an existing
banner with a new accent, and confirms that invalid appearance and rejected sound
installation preserve the previous presentation and configuration generation.

Completed build and configuration validation:

- `bash scripts/check.sh --core`: **349 tests passed**, none failed or ignored,
  including the compile-fail notification ID doctest.
- `bash scripts/check.sh`: **1,646 workspace tests passed** in 43 test executables,
  none failed or ignored; the product configuration fixture and four shared-fork
  library tests also passed. Formatting, seven updater tests and the 549-package
  dependency attribution inventory passed.
- Core dependency inspection excludes Iced, Wayland, AccessKit and audio adapters.
- Strict workspace Clippy (`--workspace --all-targets -- -D warnings`) passed.
- The shared preset contrast check passes **42 text/status/focus pairs** across
  dark and light palettes. Hover-state regression tests also check actual shared
  control styles, including explicit link/muted text and destructive controls.
- Both standards and scope reviews have no remaining material findings. Their
  contrast finding was fixed by keeping Quiet/Destructive hover backgrounds on
  the readable surface role; Normal actions use raised backgrounds with primary text.

Builds use `CARGO_TARGET_DIR=niribar/target`, `CARGO_INCREMENTAL=0`, and
`CARGO_BUILD_JOBS=1` on this host. Runtime tests use private D-Bus sessions. No
running desktop service or user configuration is changed. Fork sources and patches
are unchanged; provenance and protocol-sharing checks pass.

Real desktop fixtures passed in fresh private Niri/Sway sessions:

| Case | Scale | Result |
| --- | --- | --- |
| Dark preset | 1.0 | Passed |
| Light preset | 1.25 | Passed |

Both runs verified palette pixels in the shell and banner, native sender actions
from the banner and Center, image fallback, and keyboard focus in the Center's
AT-SPI tree. Reloading Notify switched the visible banner to the opposite preset
while the shell retained its startup palette. Screenshots were visually inspected
for consistent frames, action controls, typography and visible focus. These fixtures
do not establish full screen-reader usability or native coverage of every bar module.

Reproduce from the workspace root with GUI binaries already built:

```sh
python3 scripts/test-desktop.py --notify target/debug/canopa-notify \
  --niribar target/debug/niribar --artifacts /tmp/canopa-dark --color-scheme dark
python3 scripts/test-desktop.py --notify target/debug/canopa-notify \
  --niribar target/debug/niribar --artifacts /tmp/canopa-light \
  --color-scheme light --scale 1.25
```

Artifact directories must be new. Local evidence is in
`/tmp/canopa-appearance-dark` and `/tmp/canopa-appearance-light`; each contains
`result.json`, the initial desktop screenshot, the reloaded screenshot and service
logs. This host used Niri 26.04 (`8ed0da4`) and Sway 1.12. The optional desktop CI
workflow now runs both cases.

The combined debug-binary package passed `scripts/package-release.sh` and
`scripts/smoke-release.sh`. All four packaged executables run, the Wi-Fi provider
handshake/close succeeds, and both packaged validators accept the shipped shared
appearance example with the same source fingerprint. The package includes original
license notices for the two new shared crates. Local archive:
`/tmp/canopa-appearance-package/canopa-x86_64-unknown-linux-gnu.tar.gz`.
