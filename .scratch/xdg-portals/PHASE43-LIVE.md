# Historical Phase43 live notes — superseded

All handles below TERMINAL. Use CONTINUE.md Phase43 current and acceptance-01.json.
No live native/check process or pending user input.

# All current Phase43 handles terminal; acceptance seal pending

Normal dark68441 exit0:12 groups/26 observed terminal. Normal light27122 exit0:
12 groups/24 observed terminal. Corrected source crash78709 exit0:5 groups/15
observed terminal. Prior crash45581 exit1:2 groups/12 terminal, fixture ordering
was wrong: direct source snapshots happen before UI. The corrected test prepares
PDF overwrite approval first, then submits the held pipe. Both metadata/content
checks now pass; no Rust changes after worker02. No emergency cleanup kills.
Read the rest as historical launch details.

# Phase43 live checkpoint

Previous goal turn Phase42 was PROGRESS; source/receipts revalidated before work.
Full goal active:11 packaged,6 experimental,Print under development. New explicit
--experimental-print opts the normal desktop backend into registration; default
and packaged routing remain without Print. No installed service/config changes.

LIVE native handles: normal public formats68441 (public-normal-dark-01) and
normal source-snapshot crash45581 (crash-normal-dark-01), both current worker02
binary. Poll these SAME handles; do not edit guarded Rust/manifests/vendor while
live. Private CUPS/compositor/frontend only. All source paths/root target reused.
Native scripts /tmp/canopa-print-43/public-formats.py; fully automatic menus now.
No manual input needed. Source/build baseline7333220352 free,6GB reserve/3GB cap.

TERMINAL: validator worker02/97963 exit0:fmt3s, strict workspace Clippy10.01s,
original workspace/default/all-target/offline/incremental/jobs1 build69.05s,
134 Print/11ignored,12 Lockdown, full Portal352lib/19ignored +55integration/2ignored,
crate CWD. Lib and integration artifacts fresh=true from phase42; only main.rs
changed. Net growth14872576. Source guards unchanged through validation.
worker01/78346 exit1: IO vs anyhow result type mismatch in recovery store setup;
removed unnecessary map_err, then worker02 passed. No other Rust edits since.

IMPLEMENTATION: main.rs adds public explicit experimental-print flag, conflicts
with all helper/manual recovery modes, desktop=false rejects explicit flag. One
resolved RecoveryStore shared by Production and startup recovery. Print uses
normal Requests/Lockdown, registration before name acquisition/ready. Default
builder has no Print. Print.shutdown runs before recovery/policy teardown. No
packaging/manifest/dependency/vendor edit. ADR0084 in-validation draft exists.

PRE-FLAG NATIVE: public-harness-dark03/30099 exit0:9 groups,22 observed terminal,
real standard frontend/Print/Production/Iced/fresh file/overwrite consent. Both
cached GTK PS14-page/SVG4-up bodies exact new/replace, mismatch refusal/old hardlink,
consumed public token =>fresh Iced consent cancelled (response1), data unchanged.
Actual live GTK PS and SVG file output through real frontend/Iced also pass. No
printer delivery/real Flatpak proof. Source GTK clients/compositor/frontend/CUPS
all reused from existing checksum/stable stores; no new executables.
Failed baselines: harness-dark01/12472 exit1 assumed consumed public token returns
immediate failure; actually opens fresh direct consent. Screenshot proves it.
12 processes terminal. Fixed fixture cancels fresh dialog. harness-dark02/54587
exit1 after7 groups because agent missed40s manual SVG-menu input deadline;23
terminal, no emergency kills. Replaced manual pauses by screenshot-confirmed PS
row313/SVG344 logical coords. Only complete03 is overall acceptance. Preserve
failed receipts; not product failures. All native pre-flag handles terminal before
main.rs edits and compilation.

CURRENT scenarios: formats normal verifies default main Print absent, explicit
flag Print present, same9 public groups, pending consent withdrawn on SIGTERM,
no output, clean normal shutdown. Crash scenario passes a held anonymous pipe
through real public Print, gets real Iced overwrite consent, locates actual child
worker by parentPID/--print-worker AND matching pipe dev/inode in its FDs, kills
ONLY normal main, requires worker pidfd terminal and old target/hardlink unchanged,
no publication entry. Then restart normal backend and fresh PS prepare/print must
work; clean exit. This establishes source-snapshot full-main death only, not file
publication/CUPS transaction boundaries, power loss or actual kernel D-state.

NEXT after these handles terminal/pass: light125 public normal formats (same script
CANOPA_PRINT_TEST_SCHEME=light). Review selected-format drafts/consumed/pending UI
and actual GTK output content. Seal sources/pins/retirement/storage and update
ADR0084/map/README/roadmap/checkpoint from exact results. Docs-only changes can
proceed during native checks; no Rust edits until checks terminal. Broader Print
crash/remote-kernel/cross-restart/CUPS ambiguity/device/custom media/saved-instance/
runtime/real Flatpak/physical gates and6 experimental+final release/10 Files failures
remain. Normal explicit opt-in is not normal advertisement or completion.
