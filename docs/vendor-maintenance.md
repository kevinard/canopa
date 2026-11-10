# Upstream dependency forks

Canopa keeps every patched upstream project in a GitHub fork under
`kevinard/`, with one tracking PR per change. Nothing is vendored in this
repository any more, except the overlay recipes for separately built programs
described below, which are moving to the same forks.

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
| Optional ordinary-window Wayland drag transport | Files needs exact native input grabs, negotiated Copy/Move, retained offers and asynchronous completion after origin closure. Layer surfaces decline it. | Scoped transport and Files checks cover retained offers, transfers/Undo, scrolling, narrow tiles, selected clipboard coexistence and logical-Escape ordering. Strict peer cancellation now passes on the pinned Niri companion below; the installed compositor still reproduces the failure. Distribution selection, broader coexistence and shared-stack regressions remain open (ADR0023/0024, Files ADR0095). |
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

## Pinned source overlays

Run `python3 scripts/check-vendor.py` to verify these recipes against
`vendor/provenance.json`; add `--archives DIRECTORY` to also verify downloaded
source archives. The checker performs no downloads.

The experimental `niri-remote-touch` recipe adds contact ownership, physical
device bookkeeping and an FD-owned mapped touch transport to the complete
keyboard companion below, with the public RemoteDesktop touch implementation of
ADR0117. The Arch `canopa-compositor` package builds this recipe, so it is the
compositor every packaged session runs, despite its experimental status. Its maintained
std-only checks run in the workspace gate. See
[ADR0117](adr/0117-remote-touch-ownership-and-transport.md) and the recipe README
for the exact archive delta, transport and native acceptance scope.

The experimental `niri-input-capture-keyboard` recipe extends the complete
combined companion with an eight-file Niri delta and unchanged Smithay. It adds
private v2 keyboard routing, independent capture XKB, keymap revision handling
and source/virtual-modifier ownership. Twenty pure checks and nine private
native groups pass; exact official archive comparisons retain original notices.
Builds reuse the stable source paths/native incremental cache and recorded
compiler. The old recipe and delivered companion remain intact. Public receiver
EI and Iced keyboard integration pass six native groups in each appearance; the
recipe is accepted by explicit optional packaging, under
[ADR0111](adr/0111-input-capture-keyboard-ownership.md).

An external companion can retain a small source patch instead of another full
checkout. Its canonical patch, licences and pinned archive recipe live in
`vendor/source-overlays/` and `vendor/provenance.json`; disposable build copies
belong under `target/` or `/tmp`. This does not add an application-local fork.
The checker verifies recipe and licence hashes normally; with `--archives` it
also reproduces the exact patch and checks every changed file and its scope.

The explicit `xdg-desktop-portal-input-order` recipe pins the official frontend
1.22.1 archive and its LGPL licence. Three files add bounded, receive-ordered
Notify admission per authorized RemoteDesktop session, with worker-based setup
and the original handler checks. Concurrent and NoReplyExpected native keyboard
groups pass in both appearances; memory-error sanitizer checks also pass.
See [ADR0047](adr/0047-ordered-portal-input-forwarding.md) for scope and open gates.
The separate two-file session-owner patch pins implementation sessions to their
original unique owner and revokes them after backend loss or name replacement.
Revocation rejects further admission immediately; bounded original-owner cleanup
and a closure worker keep the frontend loop responsive. Pending creates cannot
revive after name reacquisition or early backend closure. Ten isolated-bus
scenarios pass in normal and address/undefined builds. See
[ADR0055](adr/0055-frontend-session-owner-lifetime.md) for native/canonical and
shared-gate acceptance; the installed frontend remains unchanged.
The separate one-file InputCapture phase patch preserves Active on repeated
Enable and waits for implementation signals after Release. It fixes public
Clipboard denial while actual capture continues after a no-op or rejected stale
control. The recipe's original archive/license and existing source/build paths
are retained; the complete 499-file materialization and each old/new recipe are
verified before a same-cache rebuild. Its native baseline/candidate and broader
gate results are recorded in the InputCapture ledger.
The separate one-file USB ownership patch validates the complete backend grant
against the requested IDs and access modes before recording permissions. Late
consent after request closure cannot create pending access. Finish rechecks current
visibility, opens the exact granted mode when it differs from the cache, and keeps
one sender-owned FD per device so release also works after repeated acquisition.
Existing client FD copies cannot be revoked by closing that cached copy. Private
UMockdev devices and a controllable backend reproduce the original failures;
candidate USB and broader companion acceptance are tracked in the portal ledger.
`scripts/build-xdg-portal-compat.py --archives ARCHIVES --work-dir target/NEW-SOURCES`
prepares/builds without installing or selecting the frontend. Arch packaging runs
it and installs the result at `/usr/lib/canopa/xdg-desktop-portal`; the
`canopa-portal-frontend` wrapper selects it only inside the dedicated Canopa
session. `--prepare-only`
performs source verification; `--reuse` retains the same source/build cache.
Meson/Ninja and GLib generators must already be available; `--native-file FILE`
can name explicit build-tool overrides. Canonical source inputs, recipe, tool
bindings and binary hashes are recorded. The normal debug build uses one job and
does not download dependencies or run the separately provisioned upstream suite.

The `niri-source-cancellation` recipe pins Niri 26.04 and the exact Smithay Git
revision from its lockfile. Only Smithay's grab/source lifetime code is patched.
Niri retains its GPL-3.0-or-later licence and Smithay its MIT licence. Both source
archive names, URLs and SHA-256 values are recorded. This is the compatibility
work in [ADR0024](adr/0024-niri-drag-source-lifetime.md). The private companion
passes strict cancellation, retained-offer and actual Files regressions;
the Arch recipe selected that pinned companion at a private system path until
commit 8ede03f moved it to the cumulative `niri-remote-touch` recipe, which
includes this correction.
Physical installation and broader acceptance remain open.

`python3 scripts/build-niri-compat.py --archives ARCHIVES --work-dir target/NEW-SOURCES`
prepares a new disposable upstream workspace and builds a companion without
installing it. `--prepare-only` verifies the patch and records the source bindings
without compiling. The helper changes only the exact Smithay and
smithay-drm-extras source bindings in Niri's copied manifest/lockfile; every other
locked dependency stays unchanged. Cargo compilation remains locked/offline,
single-job and nonincremental, with outputs under root `target/` or an explicit
`CARGO_TARGET_DIR`. `--fetch` explicitly fetches locked dependencies first.
`--release` selects the optimized, stripped distribution executable. `--resume`
reuses prepared sources only after checking every file against the verified
archives, canonical patch and independently recomputed source bindings; altered
or extra files are rejected. The ordinary Canopa Cargo lockfile is
not rewritten. An explicit `--target-dir` can reuse an existing companion build
cache after preserving any previous executable.

The `niri-screencast-resume` recipe independently pins Niri main
`b22eb842ca9506d4e2ff997b116031cdb3f650f8`. It resets content/cursor damage history
on consumer resume so a discarded first static-window frame cannot leave a
consumer waiting forever. It retains Niri's GPL-3.0-or-later license and original
dependency lockfile. See [ADR0043](adr/0043-screencast-frame-resume.md).

`python3 scripts/build-niri-screencast.py --archives ARCHIVES --work-dir target/NEW-SOURCES`
verifies/applies this canonical patch and builds the independent upstream
workspace, using one incremental dev compiler job and root target/ outputs.
`--prepare-only` records verified sources without building; `--toolchain NAME`
selects an explicit compiler for a controlled baseline comparison. A build receipt
records the compiler, source/recipe/lock hashes and binary checksum. Dependencies
for the original upstream lockfile must already be fetched. Neither companion
helper installs or selects the user's compositor.

The explicit `niri-desktop-compat` recipe combines that capture pin with the
source-lifetime correction ported to its original Smithay revision. It reuses the
Niri resume patch and records a separate six-file Smithay patch/licence; both
independent earlier recipes remain available. See
[ADR0045](adr/0045-combined-niri-desktop-companion.md). Select it with
`--overlay niri-desktop-compat` on the capture builder. The two builders share a
strict source-binding transform: only smithay/smithay-drm-extras become local
paths, all other locked dependencies remain unchanged, and invalid source bindings
are refused before either prepared file is written. Scoped transport, Files and
ScreenCast acceptance passes on the combined candidate; startup failures remain
recorded alongside successful reruns. Full matrix/release acceptance and compatible
distribution provisioning remain open. It is not installed or selected by the
application archive.

The explicit `niri-remote-input` recipe extends that exact combined base with
compositor-owned keyboard leases and a cached Smithay keymap identity getter.
It retains the same upstream pins, dependency graph and original licences. Its
Niri patch explicitly adds four files: null upstream hashes mean absent paths,
and archive verification rejects an existing file relabelled as an addition.
Three source-addition/scope tests run in the workspace gate. Select this recipe
with `--overlay niri-remote-input`; `--prepare-only` verifies/applies all four
patches and the same two source bindings without compilation or installation.
The registered recipe passes exact fifteen-file-entry archive materialization.
Its separately negotiated typed symbol lease resolves keysyms on the compositor
thread, acknowledges commands and retains shared modifier ownership. Sixteen
model/wire tests, twelve typed native groups, thirteen existing evdev groups and
six combined public-frontend/media/EIS groups pass on the typed candidate. Legacy
Notify wiring passes scoped native acceptance with the frontend companion above;
pointer/touch and broader acceptance remain. The recipe stays unadvertised and is
not selected by the application archive. See [ADR0046](adr/0046-session-owned-remote-input.md).

The explicit `niri-desktop-pointer` recipe combines that base with the prepared
capture-bound pointer/physical-scale stack and creator-owned recording cleanup.
Its two patches declare 22 Niri and eight Smithay files against the same original
archives and licenses. The full materialization matches all 5961 files of the
compiled candidate, normalizing only two absolute local dependency paths.
Private lifecycle/media regressions pass; normal advertisement, full matrix and
compatible distribution acceptance remain pending. See
[ADR0056](adr/0056-capture-creator-lifetime.md).

The capture builder's `--reuse` option retains a prepared workspace's source
paths and incremental Cargo identities. It verifies the recipe, paths and full
input inventory before reuse, rejecting edited, added or deleted inputs and
unexpected symlinks. Use `--prepare-only --reuse` to verify without compiling.
The explicit pointer recipe uses the same `+canopa-pointer-lease` build label as
the measured candidate; binary hashes and the creator-cleanup property identify
the exact accepted behavior. Retain baseline/candidate executables through the
shared checksum-addressed store rather than copying them into each native run.

For full archive verification, the `--archives` directory must contain all the
published crate archives and pinned source archives named in
`vendor/provenance.json`. Verification performs no downloads.

Run `bash scripts/check.sh --workspace` and `bash scripts/check.sh --core`, followed by the
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

## Experimental input capture companion

The explicit `niri-input-capture` recipe extends the desktop-pointer companion
with a private compositor-owned pointer capture lease. Its complete Niri patch
uses the same pinned archives and existing licences. Complete Niri and Smithay
patches live in this recipe; licence attribution remains shared by reference.
The old recipe and frozen binary remain
available as a baseline. Seven geometry tests, eight capture groups and 54
existing portal/input/media native groups pass on one new hash. This is pointer
foundation acceptance, not a public InputCapture backend, EI receiver, keyboard/
touch capture, Iced consent or production distribution. See
[ADR0059](adr/0059-compositor-owned-input-capture.md) and its linked ledger.

Use `--overlay niri-input-capture` explicitly with the stable prepared source
paths, recorded `--toolchain nightly` compiler and existing compositor target.
Verify the compiler identity in the receipt before rebuilding; changing it can
accumulate another dependency graph. Recipe/source identity checks still refuse
reuse after an unverified mutation. The recorded checkpoint proves the complete
5963-file tree against official archives/canonical patches with only the two
validated stable Smithay bindings. Native runs reference one checksum-addressed
executable rather than copying it into every evidence directory.

The same combined recipe now contains a forwarded-press-aware keyboard-source
teardown refinement. Smithay uses its existing seat ledger to update all source
state while sending ordinary releases only for forwarded presses; changed modifier
state is still published and no pending release action runs. An initial optional
filter and Niri caller change passed ordinary bindings but missed accessibility
grabs. The final delta restores that caller and changes one Smithay source only.
Exact complete materializations are verified before reusing the pinned nightly
cache. Native controls and current wider-gate status are in the GlobalShortcuts
preparation ledger.

The explicit `niri-global-shortcuts` recipe retains that verified Smithay patch
and adds compositor-owned shortcut leases in Niri. The complete Niri patch stays
relative to the official archive and the input-capture recipe remains unchanged.
Thirteen Niri paths add bounded FD admission/control, current-keymap reservation,
source-aware pressed recipients and ordinary/a11y/config revalidation. Actual
canonical core tests and private native results are tracked in
[ADR0066](adr/0066-compositor-owned-shortcut-leases.md) and the compositor ledger.
This does not advertise a public GlobalShortcuts backend or install a compositor.
Use the same verified stable source paths, pinned nightly and incremental cache;
`--overlay niri-global-shortcuts` selects this recipe explicitly.

## Optional CUPS client authentication candidate

The `cups-authentication` overlay pins the official OpenPrinting 2.4.20 release
archive by its published SHA-256, preserving both LICENSE and NOTICE. The shared
source verifier checks additional notice assets as well as the primary licence;
archive materialization also verifies their upstream contents.

Its eight-file candidate corrects Digest qop/default-algorithm and opaque handling,
applies parsed DigestOptions, and grows bounded client username/credential,
Base64 and hash buffers without sending truncated values. Its versioned extension
identifies those semantic fixes. Worker-only runtime selection and typed private
credential limits are implemented and under acceptance; system fallback keeps
64-byte usernames/189 combined bytes. Deployment and complete Print acceptance remain
open; see [the candidate](../vendor/source-overlays/cups-authentication/README.md).

An additional one-file patch preserves a custom range's declared bottom margin
instead of overwriting its top margin and retaining the 12.7 mm bottom fallback.
The metadata-only custom-paper checker can assert all four margins with
`--expected-margins LEFT,BOTTOM,RIGHT,TOP` in hundredths of a millimetre. Existing
authentication patch files remain unchanged. IPP backend resolution forwarding
and PDF-only driverless capability reduction are separate open compatibility work.

The same overlay preserves quoted-pair challenge values and emits complete escaped
Digest fields without escaping the hash inputs. The maintained
`scripts/test-cups-authentication.py` checks an actual sealed-image Rust worker
against a private strict HTTP/empty-IPP oracle, with optional loopback TLS. Its
published-vector and syntax self-test runs in the workspace gate. This is protocol
acceptance; real scheduler/Iced and device acceptance remain separate.

The same candidate rejects truncated HTTP response headers before authentication
or reconnect. `scripts/test-print-early-replies.py` checks public CUPS write and
request results against controlled loopback TLS peers. It uses synthetic bytes
and a refusing password callback, without a scheduler or document. Complete
rejections and replies lost before client observation remain distinct outcomes.

`python3 scripts/build-cups-authentication.py --archives ARCHIVES --work-dir target/cups-authentication-compat`
verifies all source inputs and builds only libcups.so.2 with one compiler job and
a disk reserve. Use `--prepare-only` to stop before configuration and `--reuse` to
retain the same sources/object cache. The helper installs nothing and never builds
or launches a replacement daemon. Its changed private CUPS layout is confined to
an isolated worker using public opaque APIs; it must not be injected into existing
system daemons, filters or modules. The integrated selector retains one sealed image, passes it to the metadata
dialog and CUPS workers, and loads it only inside those workers. Operator selection
uses CANOPA_PRINT_CUPS_LIBRARY; the optional bundle path is documented in the recipe.

## Owned IPP receiver fixture

The separate `cups-ipp-receiver` overlay uses the same official 2.4.20 archive and
original LICENSE/NOTICE assets. Its one-file test-only patch corrects inclusive
custom-media range membership and preserves explicitly configured MIME attributes
without duplicates. It does not change the client library or install a service.
See [the receiver recipe](../vendor/source-overlays/cups-ipp-receiver/README.md).

`scripts/build-print-ipp-receiver.py` verifies both complete source inventories and
requires a current successful client build receipt with the exact configured
`config.h`, `Makedefs` and `config.status`. The client builder binds those files;
older receipts reconfigure once, and later reuse refuses configuration drift.
The receiver links only the matching client image, compiles one C test program,
retains a 3 GB reserve and publishes its executable once by checksum. No daemon,
filter, installation or Cargo graph is built. The client builder's optional
`--min-free-bytes` selects its per-invocation reserve; its default remains 6 GB.

`scripts/test-print-ipp-receiver.py` sends eleven validation/read-only requests
to a private loopback receiver and discovery bus. It verifies both range limits,
single-dimension rejection, fidelity, the exact declared PDF formats, strict
capabilities and no jobs. Public Canopa/Iced and configured-driver delivery remain
separate checks. Always retain failing baseline evidence and verify actual loaded
images; never inject the private client into system CUPS executables.

## Optional matching IPP backend

The separate `cups-ipp-backend` overlay pins that same official archive and
original LICENSE/NOTICE, changing only `backend/ipp.c`. It preserves an explicit
resolution after PPD conversion with standard typed encoding; absence stays
absent. It is a separately compiled companion with the exact verified private
client headers, configuration and image, never the stock backend with another
library injected. The worker client and system daemon/filter libraries stay
unchanged.

`scripts/build-print-ipp-backend.py` verifies both complete source inventories,
compiles five C units in one invocation and retains the receiver builder's
3 GB reserve, 256 MB growth and 120-second bounds. It publishes once by checksum
and installs nothing. Its loader selects the matching sibling client in a package
or the matching checksum store in development. Keep the stable source cache.

`scripts/test-print-ipp-backend.py` checks three DPI values, asymmetric DPCM and
absence through the backend ABI at an owned loopback receiver. Configured-queue,
stock-filter and public Iced checks remain separate acceptance layers.
`scripts/package-print-backend.py` stages a relocatable, optional program beside
the required matching client and retains source correction and original notices.
The packaging regression verifies real loading after relocation and can exercise
the owned receiver. Release staging configures no service or printer queue.
See [the recipe](../vendor/source-overlays/cups-ipp-backend/README.md) and
[ADR0110](adr/0110-matching-ipp-backend-resolution.md) for evidence and open gates.
