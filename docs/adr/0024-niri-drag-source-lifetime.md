# Niri drag-source lifetime compatibility

Status: scoped native correction verified, 2026-10-20; pinned Arch package
selection implemented, 2026-11-02. Extends ADR0023. Physical installation and
broader acceptance remain required. The full Files goal remains active.

The GTK-only reproducer fails on installed Niri 26.04 and on an unmodified build
of its exact upstream commit, using the Smithay revision pinned by that lockfile.
It passes on Sway. Source destruction marks Smithay's source dead, but does not
retire its active grab; release still takes the normal Drop path. Removing Iced's
clipboard connection does not correct the failure.

Keep this correction in Smithay's input and Wayland source modules. Pointer and
touch grabs expose a default live-state query; a DnD grab binds that query to its
own source lifetime. Source destruction retires a dead current grab before
returning to native dispatch. Input dispatch also checks grab lifetime, and Drop
refuses a dead source. A destroyed older source must not cancel a newer live grab
or revoke an already accepted drop. No filesystem or Files policy enters this
module, and normal clipboard functionality stays available.

Maintain a small, reviewable patch and pinned primary-source recipe under root
`vendor/`, together with source checksums and the original Smithay/Niri licences.
The companion build prepares disposable upstream workspaces under root `target/`;
it does not add another supported application workspace or another vendored copy
inside an application crate. The ordinary Canopa Cargo lockfile and dependency
selection remain independent. Use the upstream Niri lockfile, replacing only its
exact Smithay source binding with the prepared patch. Builds must not silently
resolve newer dependencies.

The integrity checker validates the recipe, patch and licences, and verifies the
changed files against the pinned archives when provided. The build helper records
its source/lockfile transformations and executable identity. It produces a
candidate binary for explicit private-compositor acceptance; building never
installs or replaces the running compositor. Distribution must select the verified
compatible compositor before claiming the strict Niri gate complete.

Required evidence includes the unmodified baseline failure, GTK source/receiver
cancellation, original Iced/GTK transport and retained offers, actual Files
cancellation/clipboard workflows and the Sway control. Pointer and touch input
share the lifetime policy, but native pointer evidence alone must not be described
as physical touch acceptance. Preserve every earlier source and executable archive.
See [the issue](../../.scratch/file-manager/issues/01-niri-source-destruction-cancellation.md).

## Scoped verification

The unmodified pinned compositor reproduces the GTK-only failure. The companion
with the six-file Smithay patch passes the original five-check transport workflow
and three independent cancellation controls. The controls require receiver Leave
while the pointer is still held, following destruction of the exact native source;
release must produce no Drop. The Sway comparison still passes. In the retained
offer workflow, destruction of the older accepted source occurs during a newer
source's held grab; the newer drag subsequently drops and finishes normally.

The unchanged Files executable also passes twenty-five native checks on this
companion: eight independent GTK exchange, clipboard and windowless-receipt checks
in each appearance, plus nine dark-theme gesture checks. These include ordinary
and buffered Escape, narrow columns, shared Undo, a conflict surviving closure of
every browser, and outgoing Move completion with zero Files windows. A prior
sandbox attempt failed to create its private bus before either compositor or Files
started; the private-session rerun passes. This is scoped pointer evidence, not
physical touch, complete shared-stack or production performance acceptance.

The candidate executable and prepared sources are preserved with their exact
hashes. Its build changes only the two Smithay source bindings in the copied
upstream manifest/lockfile. The Canopa Cargo lockfile, Files executable and clean
transport probe are unchanged. `/usr/bin/niri` is unchanged and still has the
reproduced failure. No compositor was installed or replaced. The strict gate is
green for the recorded companion; distribution must select that compatible build
before release acceptance. The separate later-grab clipboard/retained-offer cause
and full product requirements remain open. See
[the verification record](../../.scratch/file-manager/niri-cancel-verification.json).

[ADR0045](0045-combined-niri-desktop-companion.md) adds a separate combined candidate
on the newer capture pin. The capture-only build reproduces this source-lifetime
failure twice; the ported correction passes the original controls, retained-offer
transport, actual Files exchange/gestures and ScreenCast on the same executable.
The older recipe remains unchanged. Scoped successes do not replace the full
matrix or release/distribution acceptance; startup failures and reruns are recorded
in the portal distribution ledger.

## Compositor selection for acceptance

The maintained runner accepts an explicit `--niri` executable and optional
`--niri-sha256`. Resolve and check that selection before creating artifacts or
starting a desktop. Each batch copies the selected executable into its private
artifact directory, checks the copied bytes, and uses that snapshot for both
Niri servers and IPC clients. Rebuilding or replacing the original path cannot
change the compositor halfway through acceptance. Record the original path,
snapshot path, version and SHA-256 in the batch summary, then check that the
snapshot is unchanged at completion. Sway controls continue to use Sway.

Without an explicit path, resolve the existing PATH once and record that choice;
never silently substitute the compatibility companion. Inventory listing needs
no compositor. Checksum mismatch, missing/nonexecutable paths and unrelated
executables fail before native startup. A source changed during snapshot handoff
also fails. Preflight and native execution share the selection interface.

Hosted desktop dispatch must supply an executable path and checksum, with the
same values used for preflight and the actual matrix. Build the transport example
alongside the application executables so the strict transport scenario is usable.
Hosted results still require actual run evidence; selecting a binary does not
prove its compatibility. The Canopa application archive does not install or
replace the session compositor. Provision the compatible compositor separately;
see the Arch package selection below. The complete matrix and physical
distribution acceptance remain open.

The serialized hosted job has a twelve-hour budget for the full maintained
inventory, replacing the earlier forty-five-minute budget. This permits a full
run without dropping required scenarios; it is not a measured completion time.

Eight maintained contract checks pass, including actual Niri execution from a
path containing spaces, survival of original-executable replacement and refusal
of changed source bytes during handoff. Explicit companion selection passes the
original transport, all three cancellation controls and both resident scenarios:
eighteen native checks. Its artifact path contains spaces and the preserved
executable hash stays unchanged. A default-PATH control selects the installed
stock binary and still fails the unchanged pre-release Leave requirement. Thus
selection does not hide that failure. Headless startup and same-runtime reopening
remain verified on the unchanged Files executable. These are local checks; no
hosted run or full shared-stack/release acceptance is established. See
[the selection evidence](../../.scratch/file-manager/niri-selection-verification.json).

## Arch package selection

The system recipe now builds this exact Niri/Smithay source pair and canonical
patch in its independently locked workspace. `canopa-compositor` installs the
executable at `/usr/lib/canopa/niri`, together with source provenance, its binary
SHA-256 and original dependency notices. It preserves `/usr/bin/niri` and the
ordinary Niri session. The dedicated Canopa session selects the private executable;
see [ADR0129](0129-supported-session-and-settings.md) and
[installation](../installation.md). The new release candidate passes the unchanged
strict independent GTK source/receiver cancellation control. This selects a
compatible distribution candidate; complete matrix, physical touch, DRM/VT and
package-manager installation/upgrade acceptance remain separate requirements.
