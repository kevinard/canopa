# Shared upstream dependency forks

Canopa maintains one copy of each patched dependency in root `vendor/`. Workspace
applications use the root Cargo patches and lockfile. The stable, non-yanked versions
below were checked directly against the official crates.io sparse index on 2026-09-26.

| Crate | Release | Upstream revision |
| --- | --- | --- |
| iced_layershell | 0.19.1 | `5e39311c8c5e652bd493a8f78e68e62b8929d9cf` (`v0.19.1`) |
| layershellev | 0.19.1 | `5e39311c8c5e652bd493a8f78e68e62b8929d9cf` (`v0.19.1`) |
| accesskit_unix | 0.24.0 | `ce8164ba92995cfa86005b6259115e08c8244253` (crate VCS metadata) |
| iced_runtime | 0.14.0 | `3997291f318a8bc06fa522f5579836fb3feb94df` (crate VCS metadata) |
| iced_winit | 0.14.0 | `3997291f318a8bc06fa522f5579836fb3feb94df` (crate VCS metadata) |
| winit | 0.30.13 | `e9809ef54b18499bb4f2cac945719ecc2a61061b` (crate VCS metadata) |
| uucore | 0.12.0 | `dc1efd89948a9ca4c78c3a4b9a6ac891019a8c69` (crate VCS metadata) |

The Iced runtime/Winit and native Winit additions were checked against the exact locked published
archives on 2026-10-18. Iced's MIT notice comes from that upstream revision; Winit retains the Apache
license in its published archive.
uucore was checked against its published archive and official sparse-index checksum
on 2026-10-25; its original MIT notice is retained.
Source and dependency-manifest changes are included in the reviewable patches.

The two layer-shell crate source trees were compared byte for byte with their tag
subdirectories, excluding Cargo-generated manifests/lockfiles and mapping
`Cargo.toml.orig` to the upstream manifest. Their MIT license is copied from that
tag. AccessKit's MIT and Apache license texts were retrieved from its recorded
revision. Published archive SHA-256 values match the official registry checksums.

[Machine-readable provenance](../vendor/provenance.json) records exact download URLs,
archive checksums, and upstream/local hashes for every path. [Patches](../vendor/patches/)
show source changes relative to the published crates. `Cargo.lock` is omitted from
vendored packages because the root lockfile governs the build; licenses and the
AccessKit patch explanation are packaging/documentation additions, recorded separately
in the inventory. No nested source checkout is required to build the local forks.

Retained behavior:

| Patch | Why it remains | Verification |
| --- | --- | --- |
| Descriptor copy progress with a per-call pipe | Files needs cooperative cancellation and byte reporting without reopening selected paths or sharing residual pipe data after failures. | The private runnable uutils proof, prepared-copy boundary tests, and linked release Copy/Move/cancel/reopen check; Files ADR0153 records scope and limitations. |
| Named Wayland output snapshots | The shell plans Bar Surfaces from output names even before a window exists; upstream does not expose this snapshot to application messages. | Shell eligible-output/surface reconciliation tests and compositor output checks. |
| Activation token context | Banner actions need the originating surface and recent input serial on the existing connection. | Notification activation tests and isolated desktop action checks. |
| Surface-owned idle inhibition | The shell's manual idle-inhibitor module needs optional protocol support, idempotent requests and cleanup when bar surfaces retire. | Shell lifecycle tests, retired-window adapter regression and `scripts/verify-idle-inhibitor.py` with a private compositor and idle daemon. |
| Retired-window cleanup | Compositor output loss may remove a backend unit before Closed is delivered; completed closes must not stay in the adapter retry queue. | The retained vendor adapter regression plus notification surface lifecycle tests. |
| Renderer redraw and relayout notifications | Asynchronous image completion must refresh idle surfaces; otherwise focused-window titles update while application icons remain blank until an unrelated event. | `scripts/verify-focused-window-icons.py` checks fresh PNG/SVG icons and cached switches on rotated and horizontal bars with a controlled, otherwise idle IPC stream. |
| AT-SPI cache signal wrapping | AddAccessible and RemoveAccessible require one structure argument; upstream 0.24.0 still sends unwrapped structures. | Actual native bus signature/accessibility tests in both applications. |
| Transient ordinary Iced windows | Files Quick Look needs a native parent before initial configuration to preserve its requested size. The shared runtime/Winit operation returns an explicit failure when unsupported; layer surfaces decline it. | Workspace all-target compilation, four fork library tests, published-archive provenance and 30 private Niri/Files native checks pass in dark/100% and light/125%, with 22 inspected captures. Full shared-stack regressions remain required (ADR 0019). |
| User-input-bound ordinary-window focus | Iced GainFocus reached an empty Wayland function; returning from an inspector could focus another Browser. | Native minimized red, wire diagnosis and overdue-reply red; normal return and expiry rejection pass four dark/100% and light/125% cases. The original transfer workflow passes both themes. Full shared regressions remain required (ADR 0021). |
| Optional ordinary-window Wayland drag transport | Files needs exact native input grabs, negotiated Copy/Move, retained offers and asynchronous completion after origin closure. Layer surfaces decline it. | Scoped transport and Files checks cover retained offers, transfers/Undo, scrolling, narrow tiles, selected clipboard coexistence and logical-Escape ordering. Strict peer cancellation now passes on the pinned Niri companion below; the installed compositor still reproduces the failure. Distribution selection, broader coexistence and shared-stack regressions remain open (ADR0023/0024, Files ADR0095). |
| Queued input across widget transitions | The ordinary-window adapter preserves the pending input tail until an outdated interface's messages apply and its tree rebuilds. Files form groups use the existing invalidation signal. | Three std-only queue regressions pass after two verified red failures; native/shared verification remains in progress (ADR 0020). |

Removed local patches:

- Popup grab serial tracking and popup dismissal: upstream 0.19.1 supplies them.
  Canopa Shell uses the upstream position constructor with explicit anchor/gravity to
  preserve its calculated top-left placement.
- Notify's `OutputOption::Compositor`: upstream's `Active` layer-surface option now
  passes a null output, so the compositor selects placement directly.
- Canopa Shell's vendored-manifest sibling path patch: the root workspace patch supplies
  the common layershellev source.
- The second AccessKit and generic panel protocol snapshots: each has one source.

Run `python3 scripts/check-vendor.py` for offline file and patch integrity. With
downloaded `NAME-VERSION.crate` files, use `--archives DIRECTORY` to also verify the
published archive checksums and contents. The checker performs no downloads. For an
intentional upgrade, inspect old patches against the new upstream source, remove
upstreamed behavior, update application calls, and regenerate the inventory and
patches together. Never update hashes just to suppress an unexpected difference.

## Pinned source overlays

An external companion can retain a small source patch instead of another full
checkout. Its canonical patch, licences and pinned archive recipe live in
`vendor/source-overlays/` and `vendor/provenance.json`; disposable build copies
belong under `target/` or `/tmp`. This does not add an application-local fork.
The checker verifies recipe and licence hashes normally; with `--archives` it
also reproduces the exact patch and checks every changed file and its scope.

The `niri-source-cancellation` recipe pins Niri 26.04 and the exact Smithay Git
revision from its lockfile. Only Smithay's grab/source lifetime code is patched.
Niri retains its GPL-3.0-or-later licence and Smithay its MIT licence. Both source
archive names, URLs and SHA-256 values are recorded. This is the compatibility
work in [ADR0024](adr/0024-niri-drag-source-lifetime.md). The private companion
passes strict cancellation, retained-offer and actual Files regressions;
compatible distribution selection and broader acceptance remain open.

`python3 scripts/build-niri-compat.py --archives ARCHIVES --work-dir target/NEW-SOURCES`
prepares a new disposable upstream workspace and builds a companion without
installing it. `--prepare-only` verifies the patch and records the source bindings
without compiling. The helper changes only the exact Smithay and
smithay-drm-extras source bindings in Niri's copied manifest/lockfile; every other
locked dependency stays unchanged. Cargo compilation remains locked/offline,
single-job and nonincremental, with outputs under root `target/`. Dependency
fetching is a separate explicit operation. The ordinary Canopa Cargo lockfile is
not rewritten. An explicit `--target-dir` can reuse an existing companion build
cache after preserving any previous executable.

For full archive verification, the `--archives` directory must contain both the
six published crate archives and the two pinned source archives named in
`vendor/provenance.json`. Verification performs no downloads.

Run `bash scripts/check.sh` and `bash scripts/check.sh --core`, followed by the
isolated desktop fixtures. Compilation alone does not establish popup grabs,
activation, output removal, fractional scaling, physical input regions, or native
screen-reader behavior. The experimental rotation prototype's vendored renderer
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

`scripts/test-vendor.py` runs library tests from temporary copies because Cargo
cannot test an excluded dependency's development targets in the root workspace.
It seeds the temporary lockfile from the root lockfile, gives the upstream Iced
test dependency the combined application's features, and rejects any active
registry dependency version absent from the root lockfile. Source files and
tracked vendor manifests are unchanged by this fixture.

The vendor test script also compiles the std-only iced_winit event-batch module
from the same disposable snapshot and runs its queue-contract regressions. Normal
workspace compilation covers its integration with the adapter; native acceptance
covers actual event delivery and view/focus transitions.
