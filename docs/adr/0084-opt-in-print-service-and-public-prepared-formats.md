# Opt-in Print service and public prepared formats

Status: scoped normal-service integration and native acceptance passed. Print remains under development
and absent from packaged routing. This does not promote the six experimental
interfaces or close the existing Files/workspace failures.

Print's controller and Production adapter need acceptance in the normal backend
process, including its shared request admission, policy relay, helper arguments,
signals and shutdown. Native library registration alone cannot establish that
lifecycle. An explicit `--experimental-print` switch now constructs Production
and registers Print before acquiring the backend name. The switch is off by
default and conflicts with helper/manual recovery modes. A build without the
desktop feature rejects this explicit opt-in before starting a service.

Print shares the normal backend's Requests and Lockdown owners. The configured
recovery location is resolved once and used by both Production and startup
review. An unavailable location rejects an explicit Print opt-in; without that
opt-in it retains the existing logged recovery failure policy. Shutdown closes
Print admission, tokens and document workers before recovery/policy teardown.
The service uses the same Rust worker and Iced helper executable as the private
adapter. No GTK implementation or dependency is introduced.

Public prepared-file acceptance uses the real standard XDG frontend and Iced
file/overwrite consent. PS/SVG bodies from the measured GTK client are delivered
with a returned token, preserving exact bytes for new and replacement files.
Mismatched formats must fail while preserving the reviewed target and its old
hardlink. A consumed public token opens fresh consent in the pinned frontend;
cancelling it leaves the prior output untouched. This differs from calling the
backend with an invalid backend token. The public contract distinguishes a valid
token from the ordinary interactive path:
[Print API](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Print.html).

The same fixture also runs real GTK applications through PreparePrint, the Iced
format selector and Print, for PostScript and single-side SVG. Its empty CUPS
scheduler, frontend, compositor, caller applications and all destinations are
private. This is file acceptance; actual printer submission, real Flatpak
identity and broader device/crash/resource/runtime gates remain separate.

Final acceptance is `target/portal-print-43/acceptance-01.json` and
`.scratch/xdg-portals/print/public-service-evidence.json`. Formatting, strict
workspace/all-target Clippy and original workspace/default/all-target incremental
compilation pass. Regular tests pass134 Print/12 Lockdown and complete Portal352
library/55 integration (19/2 ignored in full suites). Only main.rs differs from
the preceding phase's guarded inputs; no dependency/vendor/packaging edit occurred.
No fresh complete workspace execution ran; ten recorded Files failures remain.

Actual normal-service acceptance passes29 groups:12 each in dark100/light125 and
5 in the main-death/restart case. Both appearances verify default Print absence,
explicit opt-in registration, exact prepared PS/SVG new/replacement output, format
mismatch refusal, old hardlink preservation, consumed-token fresh consent, real
GTK clients and clean shutdown. A pending Iced consent helper is withdrawn on
SIGTERM; the public request receives non-success2 and creates no output. Actual
GTK PostScript output contains14 pages, labels1..7 twice; actual SVG has one side
with visible PAGE-1. Stored bodies and rendered previews were reviewed.

The abrupt-death case first obtains real public PreparePrint overwrite approval,
then submits a held readable pipe with the token. Snapshotting precedes direct
review, so the first fixture's attempt to await direct consent before EOF was
incorrect. The corrected case identifies the actual normal worker by its parent,
worker command and matching source pipe device/inode in its descriptors. Killing
only the main backend makes that worker's pidfd terminal while the input writer
remains open. The old file/hardlink bytes and inode survive and no publication
entry exists. A fresh normal backend using the same private state completes a new
PS prepare/print and exits cleanly. This is full-main death during source snapshot;
it is not publication/CUPS transaction death, power loss, uninterruptible D-state
or a cross-restart global quota proof. This host has no `/dev/fuse`; no actual
FUSE/kernel-wait claim is made.

All65 observed processes in these final native checks are terminal; guarded inputs
and executable pins match, with no emergency cleanup kills. Startup linkage has
no GTK/CUPS/qpdf dependency. Storage grows about19MB, reusing root target and all
native executables; the6GB reserve is preserved. The earlier9-group library-host
baseline passes separately. Its consumed-token assumption failure and later manual
menu-control timeout remain failed fixture receipts, not overall acceptance.
The original recovery-store IO/anyhow type mismatch was corrected before the
successful validation. No source edit occurred with validation/native checks live.

Prepared PS/SVG printer delivery, automatic Qt client behavior, advanced/custom-
media/device/saved-instance controls, publication/CUPS/backend failure and resource
acceptance, real Flatpak/physical testing, runtime packaging/default advertisement,
six other experimental interfaces and final workspace/native/release/routing remain.
The explicit switch enables integration while these gates are open. Installed
services and packaged interface/routing manifests are unchanged.
