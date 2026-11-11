# Upstream forks on GitHub

Status: done (2026-11-10)

Goal: move every patched upstream out of `vendor/` into a GitHub fork under the
personal account `kevinard`, with one reviewable PR per change, so Canopa pins
fork commits and upstreaming starts from clean topic branches. Decided
2026-11-10: personal account (no organization); forks are public.

## Layout

Local clones live in `/home/kevinard/projects/forks/<repo>` (not inside the
Canopa repository). Clone over HTTPS (`https://github.com/kevinard/<repo>.git`);
SSH prompts and hangs in agent shells. Large repositories use
`git clone --filter=blob:none`. Add the original as remote `upstream`.

| Fork | Upstream | Canopa content |
| --- | --- | --- |
| kevinard/reis | ids1024/reis | `vendor/patches/reis.patch` — done, PR #1 |
| kevinard/coreutils | uutils/coreutils | `vendor/patches/uucore.patch` (crate at `src/uucore`) — done, PR #1 |
| kevinard/winit | rust-windowing/winit | `vendor/patches/winit.patch` |
| kevinard/iced | iced-rs/iced | `iced_winit.patch`, `iced_runtime.patch` |
| kevinard/exwlshelleventloop | waycrate/exwlshelleventloop | `iced_layershell.patch`, `layershellev.patch` |
| kevinard/niri (exists already) | niri-wm/niri | `vendor/source-overlays/niri-*` |
| kevinard/smithay | Smithay/smithay | the `smithay.patch` files of those recipes |
| kevinard/xdg-desktop-portal | flatpak/xdg-desktop-portal | `vendor/source-overlays/xdg-desktop-portal-input-order` |
| kevinard/cups | OpenPrinting/cups | `vendor/source-overlays/cups-*` |

## Branch model (every fork)

- `canopa/base`: the exact upstream commit Canopa builds on today (from
  `vendor/provenance.json` or the overlay's recorded revision). Never rewritten
  except for a deliberate rebase.
- `canopa/<topic>`: one branch per change, short kebab-case name. An independent
  change branches from `canopa/base`. A change that needs another change
  branches from that topic (a stack).
- One PR per topic **inside the fork** (`gh pr create --repo kevinard/<repo>`;
  always pass `--repo`, because without it `gh` targets the upstream parent). Base
  is `canopa/base` for independent topics and the parent topic for stacked
  ones, so each PR diff shows exactly one change. PRs stay open as the tracking
  item until the change is upstreamed or dropped. Do not merge them on GitHub.
- `canopa/main`: `canopa/base` plus every topic, merged with `--no-ff` in
  dependency order. Canopa pins a commit of this branch. Its tree must equal
  what Canopa builds today (apart from intentional drops listed in
  `INVENTORY.md`, such as iced W7 and the rustfmt-only Niri hunk).
- Never push to the fork's default branch or to existing non-`canopa/` branches,
  and never open PRs or issues on the upstream repositories.

## Commits and PRs

- Commit as the configured git user. Subject in the upstream's style
  (`area: imperative summary`), body explaining what and why, then a blank line
  and `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Merge commits
  on `canopa/main` carry the same trailer.
- Each topic must build (and pass the tests it touches) on its own at its base.
- PR body sections: a first paragraph stating it is a Canopa tracking PR, its
  base, and that it is merged into `canopa/main`; then **What**, **Why** (with the
  Canopa ADRs), **Verification**, **Upstream** (status from `INVENTORY.md`:
  related upstream PRs or issues, whether it still applies to upstream main).
  End with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- Use `bash /home/kevinard/projects/canopa/scripts/cargo.sh` for Cargo builds
  (free-space guard); each clone keeps its own `target/`.

## Cargo pitfalls

- Crates that live in a multi-crate repository (iced, exwlshelleventloop)
  bring their sibling crates along from git. Patching only `iced_winit` from git
  yields two `iced_core` copies. Canopa must patch every crate of that
  repository that appears in `Cargo.lock` from the same fork commit, so the
  fork base must match every locked version (Canopa locks `iced_widget 0.14.2`;
  use iced's `0.14` branch history, not only the `0.14.0` crate commit).
- Vendored crates hold crates.io-normalized manifests. Translate any manifest
  hunk to the repository's real `Cargo.toml`.
- A fork that needs another fork (iced needing winit additions, Niri needing
  Smithay) adds a `[patch.crates-io]` or git `rev` entry pointing at the other
  fork's commit on `canopa/main`, or at the matching topic commit for a topic.

## Status (2026-11-10)

| Fork | `canopa/base` | `canopa/main` | Tracking PRs |
| --- | --- | --- | --- |
| reis | `02bc1b52` (0.7.1) | `c99f5a71` | #1 |
| coreutils | `dc1efd89` (uucore 0.12.0) | `1e4f136e` | #1 |
| winit | `e9809ef5` (0.30.13) | `5bbfe4f2` | #1–#7 |
| iced | `595af03a` (`0.14` branch, iced_widget 0.14.2) | `af379ca0` | #1–#8 |
| exwlshelleventloop | `5e39311c` (v0.19.1) | `fecb4f1a` (#6 pins the iced and winit forks; #4 records the keyboard serial) | #1–#6 |
| niri | `b22eb842` | `9389bbaa` | #1–#12 (`canopa/fixes` = base + the six fixes; features stack on it) |
| smithay | `8a270232` | `55163887` | pushed; 4 to open |
| xdg-desktop-portal | `1d20fad` (1.22.1) | `5272cf5` | #1–#11 |
| cups | `ef80eea` (v2.4.20) | `aed09bb` | pushed; 13 to open |

Every `canopa/main` tree was checked against what Canopa builds today. Known,
intended differences:
- the Niri rustfmt hunk is dropped;
- a test-only `Vec::<u32>::new()` fix in Niri `pointer_buttons.rs`;
- iced W7 kept as its own topic, which bumps futures-channel to 0.3.32;
- Smithay is bound by git `rev` instead of a local path.

The Smithay and CUPS branches are now on GitHub (they needed the token's
`workflow` scope because their bases carry older `.github/workflows` files);
their tracking PRs still have to be opened with
`/home/kevinard/projects/forks/smithay/.git/canopa-pr/push-and-open-prs.sh` and
`/home/kevinard/projects/forks/cups/.git/canopa-pr/push-and-open-prs.sh`.

The global git config sets `push.followTags = true`; push fork branches with
`--no-follow-tags`.

Findings from the split:
- Two portal fixes are already on upstream main (#1 invocation app-info ref,
  #3 session unclaim).
- The layershellev keyboard-serial gap is confirmed: keyboard-triggered
  activation gets no token without recent pointer input. Fixed 2026-11-11 in
  #4 (`canopa/main` `fecb4f1a`).
- The output-snapshot change breaks `iced_layershell --features debug`, which
  Canopa does not use.

## Canopa switched to the crate forks (2026-11-10)

Root `Cargo.toml` patches winit, iced (13 crates), exwlshelleventloop (5 crates),
reis and uucore from the forks' `canopa/main` by git `rev`. `vendor/` crate
copies, `vendor/patches`, the provenance `crates` section and the fork test
harness (`test-vendor*.py`, `vendor_test_workspace.py`) are removed; fork tests
run in the forks. The workspace gate shows only the known baseline failures.

## Canopa switched to the program forks (2026-11-10)

`vendor/` is gone. `packaging/forks.json` pins kevinard/niri, kevinard/xdg-desktop-portal
and kevinard/cups by `canopa/main` commit, codeload archive URL and SHA-256, plus
the license checksums and Niri's locked kevinard/smithay revision:

| Fork | Pinned commit | Archive SHA-256 |
| --- | --- | --- |
| niri | `9389bbaaa37ba91b9c110ebe457a2f6386e69299` | `1d6cb7383198ccb37b27e9046d120900b5b0d1b71c9eb34fbe57fe5500b5eb28` |
| xdg-desktop-portal | `5272cf5b3e0ad62a0f71213f4c87c728bd142720` | `4c12e31e8201706365fb0b84df9e4799ce99b228d06832a7db76030d8ac4f0ce` |
| cups | `aed09bb63fd0c78639527bda451cbba9bf276d79` | `979871d4c420fd55e3dfd36c61842fa08223e79716bb854674ae8b6f3fb5a60a` |

- The PKGBUILD downloads the Niri and portal archives; Cargo fetches Smithay from
  the rev in Niri's lockfile. `scripts/build-niri.py` replaces
  `build-niri-screencast.py`, `build-niri-compat.py` and the Smithay path binding;
  `--ref canopa/<topic>` builds a topic for comparison (unpinned, not packageable).
- The portal takes `subprojects/gvdb` and `subprojects/libglnx` from the 1.22.1
  release tarball (checksummed); the prepared tree equals the old recipe's 499 files.
- CUPS client, backend and receiver build from one fork tree; the git tree has
  `configure`, and only `packaging/cups.spec` (unused) is release-tarball-only.
- Packaging accepts only pinned-commit receipts and installs the fork license
  texts; provenance files record fork URL, commit and archive checksum.
- `vendor_sources.py`, `check-vendor.py`, `niri_source_bindings.py` and the
  overlay-materialization tests are deleted; `test-fork-sources.py` replaces them
  in the gate. The std-only Niri core tests read their modules from the pinned
  archive.

To move a pin, follow `docs/vendor-maintenance.md` ("To move a pin").
