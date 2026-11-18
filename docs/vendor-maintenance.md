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
| [kevinard/niri](https://github.com/kevinard/niri) | niri-wm/niri | niri-ipc | Niri `b22eb842`; the `rev` is always the compositor pin in `packaging/forks.json`, so the Shell and Settings speak the IPC of the compositor they ship with (`scripts/test-fork-sources.py` checks it) |

The layer-shell crates also pull crates.io `dpi` 0.1.2 through `winit-core`
0.31; that copy is unpatched and its types never meet winit 0.30's.

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
a link to the upstream change. Then rebuild `canopa/main`. The reason for each change and its upstream
status are in the [fork inventory](fork-inventory.md).

### Conventions

- Local clones live outside this repository (for example
  `~/projects/forks/<repo>`). Clone over HTTPS; large repositories with
  `git clone --filter=blob:none`. Add the original project as remote `upstream`.
- Open tracking PRs inside the fork: `gh pr create --repo kevinard/<repo>`.
  Always pass `--repo`; without it `gh` targets the upstream parent. The PR base
  is `canopa/base` for an independent topic and the parent topic for a stacked
  one, so each diff shows exactly one change. Do not merge the PRs on GitHub.
- Never push to the fork's default branch or to branches outside `canopa/`, and
  never open PRs or issues on the upstream repositories without a deliberate
  decision to upstream.
- Commit subjects follow the upstream project's style (`area: imperative
  summary`), with a body explaining what and why. A PR body states that it is a
  Canopa tracking PR, its base, and that it is merged into `canopa/main`, then
  **What**, **Why**, **Verification** and **Upstream** (related upstream issues
  or PRs, and whether the change still applies to upstream main).
- Each topic must build, and pass the tests it touches, on its own at its base.
- If `push.followTags` is set globally, push fork branches with `--no-follow-tags`.
- Vendored crates carry crates.io-normalized manifests; translate any manifest
  change to the repository's real `Cargo.toml`. A fork that needs another fork
  (iced needing winit, Niri needing Smithay) points at the other fork's
  `canopa/main` commit (or the matching topic commit) with a `[patch]` or git
  `rev` entry.

## Retained behavior

Each row is one or more tracking PRs in the fork named in the table above.

| Patch | Why it remains |
| --- | --- |
| Exported Wayland dialog parents and modal hints | The FileChooser portal needs out-of-process ownership before initial configure. Optional xdg-foreign-v2/xdg-dialog-v1 objects remain owned by the child. |
| Focus-scoped keyboard shortcut inhibition | A shortcut editor must receive input without freeing existing registrations. The ordinary-window adapter tracks actual permission/focus per seat and retires retained guards with the native owner; layer surfaces decline it. |
| Source-owned shortcut recording | Capture base symbols and actual post-press native modifiers before widgets, with generation-checked cancellation and consumed-cycle cleanup. The Iced result stream remains bounded and explicit Apply retains permission ownership. |
| Descriptor copy progress with a per-call pipe | Files needs cooperative cancellation and byte reporting without reopening selected paths or sharing residual pipe data after failures. |
| Named Wayland output snapshots | The shell plans Bar Surfaces from output names even before a window exists; upstream does not expose this snapshot to application messages. |
| Activation token context | Banner actions need the originating surface and recent input serial on the existing connection. |
| Surface-owned idle inhibition | The shell's manual idle-inhibitor module needs optional protocol support, idempotent requests and cleanup when bar surfaces retire. |
| Retired-window cleanup | Compositor output loss may remove a backend unit before Closed is delivered; completed closes must not stay in the adapter retry queue. |
| Renderer redraw and relayout notifications | Asynchronous image completion must refresh idle surfaces; otherwise focused-window titles update while application icons remain blank until an unrelated event. |
| Transient ordinary Iced windows | Files Quick Look needs a native parent before initial configuration to preserve its requested size. The shared runtime/Winit operation returns an explicit failure when unsupported; layer surfaces decline it. |
| Portal application activation handoff | AppChooser must consume incoming activation and produce a fresh token from recent input on its own focused window. |
| User-input-bound ordinary-window focus | Iced GainFocus reached an empty Wayland function; returning from an inspector could focus another Browser. |
| Retired-window configure and close | The input-bound activation keeps a closed source window's role alive until the compositor answers, so a configure or close can arrive for a window winit already retired; upstream aborts with "got configure for dead window." (winit#9). |
| Optional ordinary-window Wayland drag transport | Files needs exact native input grabs, negotiated Copy/Move, retained offers and asynchronous completion after origin closure. Layer surfaces decline it. An optional drag icon (RGBA on a `wl_shm` surface) follows the pointer (winit#8, iced#9). |
| Wheel-scrolled text inputs | A long value (a path, a file name) can be read by scrolling the field sideways with a horizontal wheel, touchpad or Shift+wheel, without moving the caret; upstream `text_input` ignores wheel events (iced#10). |
| Queued input across widget transitions | The ordinary-window adapter preserves the pending input tail until an outdated interface's messages apply and its tree rebuilds. Files form groups use the existing invalidation signal. |
| Finite EIS server reads | RemoteDesktop needs to observe revocation between bounded reads and bound pending bytes/received FDs. The worker still needs its own frame budget. |

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
native comparisons.

The topics that make up the pin, each a tracking PR in the fork:

| Fork ref | Base | Canopa use |
| --- | --- | --- |
| `canopa/fixes` | `canopa/base` | The general bug fixes; the feature topics below stack on it |
| `canopa/screencast-resume` | `canopa/fixes` | Static ScreenCast frames after a consumer pauses and resumes |
| `canopa/dnd-source-lifetime` (with Smithay `canopa/dnd-source-lifetime`) | `canopa/fixes` | Drag cancellation when the source is destroyed (Files drag and drop) |
| `canopa/remote-input-keyboard`, `canopa/remote-input-pointer`, `canopa/remote-input-touch` | `canopa/fixes` | Session-owned RemoteDesktop keyboard, pointer and touch input |
| `canopa/input-capture`, `canopa/input-capture-keyboard` | `canopa/fixes` | Compositor-owned InputCapture pointer and keyboard leases |
| `canopa/global-shortcuts` | `canopa/fixes` | Compositor-owned GlobalShortcuts leases |
| `canopa/viewport-ipc` (#14) | `canopa/base` | Minimap visible-area frame |
| `canopa/window-parent-ipc` (#15) | `canopa/base` | Taskbar dialog grouping; the Shell's `niri-ipc` comes from the fork pin |
| `canopa/spawn-placement` (#16) | `canopa/base` | Window session restore and workspace templates |
| `canopa/overview-typing` (#17) | `canopa/global-shortcuts` | Type to search from the overview (`overview { emit-typed-text; }` in the shipped defaults) |
| `canopa/touch-gestures` (#18) | `canopa/remote-input-touch` | Touchscreen strip and overview gestures |
| `canopa/window-preview` (#19) | `canopa/base` | Taskbar tooltip thumbnails (`org.canopa.Niri.WindowPreview1`) |
| `canopa/pinned-columns` (#20) | `canopa/base` | Pinned columns in the minimap, taskbar menu and `Mod+P` defaults |
| `canopa/titlebars` ([#22](https://github.com/kevinard/niri/pull/22)) | `canopa/base` | Negotiated SSD titlebars: compositor-owned close/maximize controls, dragging and double-click maximize, configurable style and layout, `draw-titlebar` rule |
| `canopa/keep-last-frame` (#21, with Smithay `canopa/keep-last-frame`, kevinard/smithay#5) | `canopa/base` | `NIRI_KEEP_LAST_FRAME_ON_EXIT=1` leaves the login screen's last frame on screen for the desktop session (`DRM_IOCTL_MODE_CLOSEFB`) |

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
the fork source. The four frontend changes are listed in the [fork inventory](fork-inventory.md).

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
`scripts/test-print-backend-package.py`. See [printing](printing.md) for how the runtime is selected and packaged.

## Verification after a pin change

After changing a fork pin, run `bash scripts/check.sh --workspace` and
`bash scripts/check.sh --core`, followed by the isolated desktop fixtures. Compilation alone does not establish popup grabs,
activation, output removal, fractional scaling, physical input regions, or native
screen-reader behavior.

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
