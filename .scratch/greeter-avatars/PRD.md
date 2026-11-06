# Greeter user avatars

Status: implemented and validated; full workspace success is not claimed.

The shared avatar widget now accepts a prepared selected-user thumbnail and an
initials fallback. Greeter source priority is explicit `user-avatar`, AccountsService
`IconFile`, then `avatar-dir/USERNAME` (default /var/lib/AccountsService/icons),
followed by the static view avatar or initials. The directory works with NSS and
manual users without AccountsService. Per-user entries do not add accounts or
change authentication. No home scan, remote fetch or image setting operation is
performed. Canonical username and active authentication input stay independent.
`user-avatars #false` disables the widget, file loads and IconFile queries; explicit
widget/layout lists must place avatar. Packaged defaults include the avatar.

Only the selected user is loaded. One worker coalesces pending requests, cancels
superseded work and caches eight 128x128 RGBA thumbnails/misses (512 KiB pixel
storage). The consumer checks generations before accepting results and clears the
old image on selection changes. No file polling, persistent cache or animation
loop is added. Preview reads only an explicitly configured selected-user asset,
without account queries or commands. list-users optionally exposes icon_file
metadata without opening image files.

The existing NSS supervisor was extracted into a greeter-local helper module and
reused for avatar decoding: two-second deadline, nonblocking bounded output,
process-group kill/reap on timeout/cancellation, parent-death SIGKILL, and no
greetd/notification endpoints. The fresh helper protects dumps before decoding.
canopa-assets applies per-file 2 MiB / 2048 dimension / 4 megapixel limits and a
32 MiB best-effort decoder allocation limit; three attempts, 6 MiB encoded and
48 MiB admitted source RGBA bound each request. Output is cropped/resized to
128x128 before transfer, then circularly masked by the shared renderer. No new
third-party dependency or vendor fork was introduced.

Validation:

- **44 focused tests passed** in focused-final.log: 16 greeter library, 5 private
  AccountsService, 3 avatar integration, 2 compositor, 2 configuration, 4 NSS,
  8 installed-session and 4 shared-presentation tests. Coverage includes optional/
  invalid IconFile metadata, disabled queries, path ordering, extensionless PNGs,
  center cropping and alpha, a valid encoded image exceeding the byte budget,
  dimension/nonregular-file rejection, initials/static fallback, preview isolation,
  cache eviction, cancellation/stale results and password-input isolation.
- **35 native checks passed under Cage and 35 under Weston** (cage-final/ and
  weston-final/), including actual rendered source-color assertions, selected
  canonical greetd username, password-stage image persistence, per-user priority,
  NSS directory images and disabling avatars. All original authentication/session
  cases passed. Screenshots use synthetic solid-color image files.
- **23 lockscreen native regressions passed**, lockscreen/: logind/PAM lifecycle,
  prompt factors, crash/failure locking, cancellation, widgets and layout.
- Strict workspace Clippy passed (clippy.log). Root gate tooling preflights passed.
  Unified configuration, settings-portal activation/read/reload, real-libc private
  NSS enumeration and vendor regression tests passed. The prior portal host
  failure did not recur in this run. Dependency attribution is current at 604
  entries; the digest changed for the existing workspace asset dependency.
  Packaged configuration, Rust formatting, whitespace, Python syntax and release
  documentation staging (38 files / 56 offline links) passed.

The required workspace gate ran in workspace.log. A new test-only enum spelling
and Clippy from_ref suggestion were corrected; the final focused suite and strict
Clippy passed. After its lint stop, the remaining gate checks continued separately
rather than repeating tooling preflights. workspace-tests.log reached the already
recorded Files recovery_catalog test failure:
`pages_remain_loaded_and_saved_identity_survives_active_undo_expiry` reports
"Recovery catalog did not settle". The Files operations crash fixture also ran for
more than 60 seconds before completing; its 4000-file preparation was still making
progress. No Files behavior was changed. This does not establish a passing full
workspace/release gate.

The first full Cage attempt (cage/ and cage.log) showed correct images but received
no fixture password answer in the override case and timed out. The cause remains
unconfirmed. An isolated avatar/account subset (probe/) and both final complete
native matrices passed. The subset is identified explicitly and is not counted
as a full matrix. Physical seat/VT/DRM, deployed PAM and remote-directory providers
remain target-machine acceptance. Avatar files/misses are cached for this frontend
lifetime and update when it restarts or an evicted entry is loaded again. Historical
compositor benchmarks were not rerun or relabeled as avatar measurements.

No host account/avatar files, services, NSS/PAM policy or display-manager
configuration were modified. All image/account fixture evidence is synthetic.
