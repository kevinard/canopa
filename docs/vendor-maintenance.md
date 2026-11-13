# Upstream dependency forks

Canopa keeps every patched upstream project in a GitHub fork under
`kevinard/`, with one tracking PR per change. Nothing is vendored in this
repository: Rust crates come from the forks through Cargo, and the separately
built compositor, portal frontend and CUPS components are built from pinned
fork source archives (see [Separately built programs](#separately-built-programs)).

## Rust crate forks

The root `Cargo.toml` patches these crates from git, pinned by `rev` to the
fork's `canopa/main` branch; `Cargo.lock` records the exact commits.

| Fork | Upstream | Crates taken from the fork | Base |
| --- | --- | --- | --- |
| [kevinard/winit](https://github.com/kevinard/winit) | rust-windowing/winit | winit, dpi | 0.30.13 (`e9809ef5`) |
| [kevinard/iced](https://github.com/kevinard/iced) | iced-rs/iced | iced and all 12 `iced_*` crates Canopa locks | `0.14` branch at `595af03a` (iced_widget 0.14.2) |
| [kevinard/exwlshelleventloop](https://github.com/kevinard/exwlshelleventloop) | waycrate/exwlshelleventloop | iced_layershell, iced_layershell_macros, iced_exdevtools, layershellev, waycrate_xkbkeycode | v0.19.1 (`5e39311c`) |
| [kevinard/reis](https://github.com/kevinard/reis) | ids1024/reis | reis | 0.7.1 (`02bc1b52`) |
| [kevinard/coreutils](https://github.com/kevinard/coreutils) | uutils/coreutils | uucore, uucore_procs | uucore 0.12.0 (`dc1efd89`) |

Every fork uses the same branches:
- `canopa/base` is the exact upstream commit.
- `canopa/<topic>` holds one change, from the base or stacked on the topic it
  needs. Its PR inside the fork shows exactly that change and stays open until
  the change is upstreamed or dropped.
- `canopa/main` is the base with every topic merged. Canopa pins it.

Crates that share a repository must all come from the same fork commit. Taking
only `iced_winit` from git would build a second, incompatible `iced_core`. When
Canopa locks a crate from such a repository, add it to the patch list.

To change a fork:
1. Work on its topic branch, or create a new topic and PR.
2. Run the fork's own tests (`cargo test` in the fork; the PR records which).
3. Merge the topic into `canopa/main` with `--no-ff`, and push.
4. Update the `rev` in `Cargo.toml`, run `cargo update -p <crate>`, and
   regenerate `docs/dependency-attributions.md`. That inventory names each
   fork crate by its fork URL and commit.

To upgrade upstream, rebase the topics onto the new upstream commit as a new
`canopa/base`. Drop topics that upstream now provides, and close their PRs with
a link to the upstream change. Then rebuild `canopa/main`. Contribution
conventions are in `.scratch/forks/GITHUB-FORKS.md`, and the reasons for each
change are in `.scratch/forks/INVENTORY.md`.

## Retained behavior

Each row is one or more tracking PRs in the fork named in the table above.

| Patch | Why it remains | Verification |
| --- | --- | --- |
| Exported Wayland dialog parents and modal hints | The FileChooser portal needs out-of-process ownership before initial configure. Optional xdg-foreign-v2/xdg-dialog-v1 objects remain owned by the child. | ADR0026 and the portal implementation ledger record focused, native and shared verification. |
| Focus-scoped keyboard shortcut inhibition | A shortcut editor must receive input without freeing existing registrations. The ordinary-window adapter tracks actual permission/focus per seat and retires retained guards with the native owner; layer surfaces decline it. | ADR0068; Portal 213 library/55 integration, 22 final editor groups, three retained-handle/window rounds and six shared desktop suites pass. The original workspace retains the same ten Files failures; other Rust/post stages pass. |
| Source-owned shortcut recording | Capture base symbols and actual post-press native modifiers before widgets, with generation-checked cancellation and consumed-cycle cleanup. The Iced result stream remains bounded and explicit Apply retains permission ownership. | ADR0069; ten policy and three standalone runtime tests plus strict whole-workspace Clippy pass. The actual normal app passes36 public native groups across both appearances; three probes confirm12 key/modifier serial pairs. The current shared gate, native retained-handle and further layout acceptance remain pending. |
| Descriptor copy progress with a per-call pipe | Files needs cooperative cancellation and byte reporting without reopening selected paths or sharing residual pipe data after failures. | The private runnable uutils proof, prepared-copy boundary tests, and linked release Copy/Move/cancel/reopen check; Files ADR0153 records scope and limitations. |
| Named Wayland output snapshots | The shell plans Bar Surfaces from output names even before a window exists; upstream does not expose this snapshot to application messages. | Shell eligible-output/surface reconciliation tests and compositor output checks. |
| Activation token context | Banner actions need the originating surface and recent input serial on the existing connection. | Notification activation tests and isolated desktop action checks. |
| Surface-owned idle inhibition | The shell's manual idle-inhibitor module needs optional protocol support, idempotent requests and cleanup when bar surfaces retire. | Shell lifecycle tests, retired-window adapter regression and `scripts/verify-idle-inhibitor.py` with a private compositor and idle daemon. |
| Retired-window cleanup | Compositor output loss may remove a backend unit before Closed is delivered; completed closes must not stay in the adapter retry queue. | The retained vendor adapter regression plus notification surface lifecycle tests. |
| Renderer redraw and relayout notifications | Asynchronous image completion must refresh idle surfaces; otherwise focused-window titles update while application icons remain blank until an unrelated event. | `scripts/verify-focused-window-icons.py` checks fresh PNG/SVG icons and cached switches on rotated and horizontal bars with a controlled, otherwise idle IPC stream. |
| Transient ordinary Iced windows | Files Quick Look needs a native parent before initial configuration to preserve its requested size. The shared runtime/Winit operation returns an explicit failure when unsupported; layer surfaces decline it. | Workspace all-target compilation, four fork library tests, published-archive provenance and 30 private Niri/Files native checks pass in dark/100% and light/125%, with 22 inspected captures. Full shared-stack regressions remain required (ADR 0019). |
| Portal application activation handoff | AppChooser must consume incoming activation and produce a fresh token from recent input on its own focused window. | ADR0028; strict Wayland input capture, serial-correlated bounded replies and seven private OpenURI/native checks per theme pass; full shared gate remains required. |
| User-input-bound ordinary-window focus | Iced GainFocus reached an empty Wayland function; returning from an inspector could focus another Browser. | Native minimized red, wire diagnosis and overdue-reply red; normal return and expiry rejection pass four dark/100% and light/125% cases. The original transfer workflow passes both themes. Full shared regressions remain required (ADR 0021). |
| Optional ordinary-window Wayland drag transport | Files needs exact native input grabs, negotiated Copy/Move, retained offers and asynchronous completion after origin closure. Layer surfaces decline it. An optional drag icon (RGBA on a `wl_shm` surface) follows the pointer (winit#8, iced#9; Files ADR 0170). | Scoped transport and Files checks cover retained offers, transfers/Undo, scrolling, narrow tiles, selected clipboard coexistence and logical-Escape ordering. Strict peer cancellation now passes on the pinned Niri companion below; the installed compositor still reproduces the failure. Distribution selection, broader coexistence and shared-stack regressions remain open (ADR0023/0024, Files ADR0095). |
| Wheel-scrolled text inputs | A long value (a path, a file name) can be read by scrolling the field sideways with a horizontal wheel, touchpad or Shift+wheel, without moving the caret; upstream `text_input` ignores wheel events (iced#10; Files ADR 0171). | Eleven `iced_widget` unit tests cover overflow, clamping, Shift, capture and reset; Canopa Files builds against the pin. Native checks in Files remain pending. |
| Queued input across widget transitions | The ordinary-window adapter preserves the pending input tail until an outdated interface's messages apply and its tree rebuilds. Files form groups use the existing invalidation signal. | Three std-only queue regressions pass after two verified red failures; native/shared verification remains in progress (ADR 0020). |
| Finite EIS server reads | RemoteDesktop needs to observe revocation between bounded reads and bound pending bytes/received FDs. The worker still needs its own frame budget. | ADR0046; original8192-byte read red, four staged socket regressions and real libei interoperability pass. Workspace adoption checks are recorded in the portal ledger; native input/session acceptance remains open. |

Removed local patches:

- AT-SPI cache signal wrapping (accesskit_unix): upstream 0.24.1 (#819) wraps the
  AddAccessible/RemoveAccessible payloads. Repeat Notify's signature test, the
  Shell native accessibility test and an Orca replacement/retirement check on
  accesskit updates.
- Popup grab serial tracking and popup dismissal: upstream 0.19.1 supplies them.
  Canopa Shell uses the upstream position constructor with explicit anchor/gravity to
  preserve its calculated top-left placement.
- Notify's `OutputOption::Compositor`: upstream's `Active` layer-surface option now
  passes a null output, so the compositor selects placement directly.
- Canopa Shell's vendored-manifest sibling path patch: the root workspace patch supplies
  the common layershellev source.
- The second AccessKit and generic panel protocol snapshots: each has one source.

## Separately built programs

`packaging/forks.json` pins three forks whose programs are built outside the
Canopa Cargo workspace. Each entry records the fork repository, the pinned
`canopa/main` commit, the upstream base commit, the GitHub source archive
(`https://codeload.github.com/kevinard/REPO/tar.gz/COMMIT`), its SHA-256 and the
SHA-256 of each original license text. The Arch recipe downloads the same
archives with the same checksums; `scripts/test-fork-sources.py` (workspace
gate) keeps the two in step.

| Fork | Upstream | Base | Builder | Installed as |
| --- | --- | --- | --- | --- |
| [kevinard/niri](https://github.com/kevinard/niri), with [kevinard/smithay](https://github.com/kevinard/smithay) from Niri's lockfile | niri-wm/niri, Smithay/smithay | Niri `b22eb842`, Smithay `8a270232` | `scripts/build-niri.py` | `/usr/lib/canopa/niri` (`canopa-compositor`) |
| [kevinard/xdg-desktop-portal](https://github.com/kevinard/xdg-desktop-portal) | flatpak/xdg-desktop-portal | 1.22.1 (`1d20fadc`) | `scripts/build-xdg-portal-compat.py` | `/usr/lib/canopa/xdg-desktop-portal` (`canopa`) |
| [kevinard/cups](https://github.com/kevinard/cups) | OpenPrinting/cups | v2.4.20 (`ef80eeae`) | `scripts/build-cups-authentication.py`, `build-print-ipp-backend.py`, `build-print-ipp-receiver.py` | optional release bundle `lib/canopa/print/` |

`python3 scripts/fork_sources.py fetch NAME... --archives DIR` downloads and
verifies the pinned archives. Every builder:
- verifies the archive checksum before extracting it into a new `--work-dir`
  beneath root `target/` (or `/tmp`); `--download` fetches a missing archive;
- records every source file's SHA-256 in `source-inputs.json` and writes a
  `canopa-build.json` receipt with the fork origin (repository, branch, commit,
  base, archive URL and SHA-256), the inventory digest, the build commands and
  compiler, the binary SHA-256 and the original license texts it found;
- keeps stable source paths and build caches with `--reuse`, but refuses a
  different commit, archive or location and any edited, added, deleted or
  symlinked input;
- installs and selects nothing.

Packaging accepts only receipts of the pinned commit. `scripts/install-system.py`
installs the license texts from the fork sources under
`/usr/share/licenses/PACKAGE/` and writes `compositor-provenance.json` or
`portal-frontend-provenance.json` under `/usr/share/canopa/`: the fork,
repository, branch, commit, base, archive URL and SHA-256 (with the Smithay
commit, or the portal's subproject archive) and the binary SHA-256. The release
bundle packagers record the same origin in their manifests.

To move a pin:
1. Merge the change into the fork's `canopa/main` (see the fork workflow above).
2. Download `https://codeload.github.com/kevinard/REPO/tar.gz/COMMIT`, then
   update `commit`, `archive`, `url`, `archive_root` and `archive_sha256` in
   `packaging/forks.json` and the matching `_niri_commit`/`_portal_commit` and
   `sha256sums` entries in `packaging/arch/PKGBUILD.in`.
3. For Niri, update `cargo_git.smithay.commit` when the lockfile moves Smithay.
   Update notice checksums only after reviewing a changed license text.
4. Run `python3 scripts/test-fork-sources.py`, build with the commands below,
   and run the acceptance in [Verification after a pin change](#verification-after-a-pin-change).

### Niri compositor

```sh
python3 scripts/build-niri.py --archives ARCHIVES --download --work-dir target/NEW-SOURCES --fetch [--release]
```

Cargo builds Niri's own workspace from its lockfile with
`--locked --offline -p niri --bin niri --jobs 1`. `--fetch` first runs
`cargo fetch --locked`, which retrieves Smithay at the kevinard/smithay revision
recorded in that lockfile. The pinned build refuses a lockfile whose git sources
are not exactly the pinned Smithay fork. `--release` builds the optimized,
non-incremental executable with stripped symbols that the Arch
`canopa-compositor` package installs; the default is an incremental dev build.
`NIRI_BUILD_COMMIT` is the fork commit, so `niri --version` names it. Outputs go
to `--target-dir` (default `target/niri-fork`); `--toolchain NAME` selects an
explicit compiler for a controlled comparison and the receipt records it.

Niri's GPL-3.0-or-later text comes from the archive and Smithay's MIT text from
its Cargo git checkout; both are checked against `packaging/forks.json`. The
Arch package also collects every other dependency notice with
`scripts/dependency-attributions.py`.

`--ref REF` builds another fork branch, tag or commit instead of the pin. The ref
is resolved with `git ls-remote`, its archive is downloaded and its checksum
recorded, and the receipt is marked unpinned, so packaging refuses it. Use it for
native comparisons with the earlier companions. Topic branches stack on
`canopa/fixes` (the base with every bug fix), so each is the closest equivalent
of a former recipe rather than an identical tree:

| Former recipe | Fork ref | ADR |
| --- | --- | --- |
| `niri-screencast-resume` | `canopa/screencast-resume` | [0043](adr/0043-screencast-frame-resume.md) |
| `niri-source-cancellation`, `niri-desktop-compat` | `canopa/dnd-source-lifetime` (Smithay `canopa/dnd-source-lifetime`), `canopa/fixes` | [0024](adr/0024-niri-drag-source-lifetime.md), [0045](adr/0045-combined-niri-desktop-companion.md) |
| `niri-remote-input` | `canopa/remote-input-keyboard` | [0046](adr/0046-session-owned-remote-input.md) |
| `niri-desktop-pointer` | `canopa/remote-input-pointer` | [0056](adr/0056-capture-creator-lifetime.md) |
| `niri-input-capture` | `canopa/input-capture` | [0059](adr/0059-compositor-owned-input-capture.md), [0065](adr/0065-keyboard-source-release-suppression.md) |
| `niri-global-shortcuts` | `canopa/global-shortcuts` | [0066](adr/0066-compositor-owned-shortcut-leases.md) |
| `niri-input-capture-keyboard` | `canopa/input-capture-keyboard` | [0111](adr/0111-input-capture-keyboard-ownership.md) |
| `niri-remote-touch` (packaged) | `canopa/main` (the pin) | [0117](adr/0117-remote-touch-ownership-and-transport.md) |

The std-only checks `scripts/test-remote-touch-core.py` (workspace gate) and
`scripts/test-input-capture-keyboard-core.py` read their pure modules from the
pinned archive and compile them with `rustc --test`, without another Cargo
graph. The fork's own tests run in the fork; its PRs record them.

### Portal frontend

```sh
python3 scripts/build-xdg-portal-compat.py --archives ARCHIVES --download --work-dir target/NEW-SOURCES [--release]
```

A git archive of the fork lacks the `gvdb` and `libglnx` Meson subprojects that
the official release tarball bundles. The builder takes the fork archive and
adds only `subprojects/gvdb/` and `subprojects/libglnx/` from the release
tarball of the fork base (`xdg-desktop-portal-1.22.1.tar.xz`, checksummed in the
same pin), refusing any overlap with fork files. Meson runs with
`--wrap-mode=nodownload`. Both inputs are checksummed archives and no patch file
remains in this repository. The cost is that the subprojects follow the base
release rather than the fork's `.wrap` revisions, so moving the fork to a new
upstream release also moves `subprojects` to that release's tarball. The
release tarball of 1.22.1 equals the fork base tree plus those 77 subproject
files, so the prepared tree is the same 499 files the former recipe produced.

The build uses one Ninja job, disables the upstream test suite and documentation,
and records the compilers; `--native-file FILE` names explicit build-tool
overrides. Arch packaging installs the result at
`/usr/lib/canopa/xdg-desktop-portal`; the `canopa-portal-frontend` wrapper selects
it only inside the dedicated Canopa session. Its LGPL `COPYING` text comes from
the fork source. Scope and acceptance of the four changes are in ADR
[0047](adr/0047-ordered-portal-input-forwarding.md),
[0055](adr/0055-frontend-session-owner-lifetime.md),
[0063](adr/0063-input-capture-frontend-phase.md) and the USB portal ledger.

### CUPS client, IPP backend and receiver

```sh
python3 scripts/build-cups-authentication.py --archives ARCHIVES --download --work-dir target/cups-authentication-compat
python3 scripts/build-print-ipp-backend.py --client-build target/cups-authentication-compat/canopa-build.json --work-dir target/print-ipp-backend-compat/maintained
python3 scripts/build-print-ipp-receiver.py --client-build target/cups-authentication-compat/canopa-build.json --work-dir target/print-ipp-receiver
```

The fork tree contains the generated `configure` script, so the client builds
from the git archive directly. The release tarball differs from the base tree
only by `packaging/cups.spec`, an RPM spec that no build step reads. The client
builder configures once and builds only `libcups.so.2` with one job, `-O0 -g0`
and a 6 GB free-space reserve (`--min-free-bytes`), binding the exact
`config.h`, `Makedefs` and `config.status`. The backend and receiver builders
compile from that same verified source tree, against its headers and client
image, so all three always come from one fork commit; they publish each program
once by checksum under `target/native-test-snapshots/` and keep a 3 GB reserve.
Nothing builds or launches a daemon, filter or service, and the private client
must never be injected into system CUPS executables.

`scripts/package-print-runtime.py` and `scripts/package-print-backend.py` stage
the optional client and backend with the original Apache-2.0 `LICENSE` and CUPS
`NOTICE` from the fork source and a `CHANGES.md` naming the fork commit, its
base and the compare link. Protocol and packaging checks:
`scripts/test-cups-authentication.py` (its `--self-test` runs in the workspace
gate), `scripts/test-print-early-replies.py`, `scripts/test-print-ipp-receiver.py`,
`scripts/test-print-ipp-backend.py`, `scripts/test-print-runtime-package.py` and
`scripts/test-print-backend-package.py`. See [the Print runtime guide](print-runtime.md)
and ADRs [0101](adr/0101-process-private-print-authentication-runtime.md),
[0108](adr/0108-maintained-ipp-receiver.md) and
[0110](adr/0110-matching-ipp-backend-resolution.md).

## Verification after a pin change

After changing a fork pin, run `bash scripts/check.sh --workspace` and
`bash scripts/check.sh --core`, followed by the isolated desktop fixtures. Compilation alone does not establish popup grabs,
activation, output removal, fractional scaling, physical input regions, or native
screen-reader behavior. The experimental rotation prototype's own renderer copy
remains outside the supported workspace.

After building root debug binaries, run these fixtures sequentially:

```sh
python3 scripts/test-desktop.py --notify target/debug/canopa-notify --shell target/debug/canopa-shell --artifacts /tmp/canopa-notifications-new
python3 scripts/test-fork-desktop.py --shell target/debug/canopa-shell --artifacts /tmp/canopa-forks-new
python3 scripts/verify-overlay-placement.py --binary target/debug/canopa-shell --output /tmp/canopa-placement-new --edge top bottom left right --scale 1.25
python3 scripts/verify-focused-window-icons.py --binary target/debug/canopa-shell --output /tmp/canopa-focused-icons-new
python3 scripts/verify-idle-inhibitor.py --binary target/debug/canopa-shell --output /tmp/canopa-idle-sway-new
python3 scripts/verify-idle-inhibitor.py --binary target/debug/canopa-shell --compositor niri --output /tmp/canopa-idle-niri-new
```

Each artifact directory must be new. The first, third and fourth fixtures use
Niri; the second uses disposable Sway outputs to remove and restore every output
without terminating the compositor. It checks actual rendered frames and Wayland popup
grabs/dismissal. These are local adapter checks; Orca and physical monitor/device
acceptance remain separate procedures.
