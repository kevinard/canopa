# Phase96: Niri integration prepared for review

User explicitly chose Prepare Niri integration for review instead of returning
to Print. Full goal ACTIVE; Print stays deferred. No installation, service reload,
restart or live routing change is authorized by that choice.

Review directory: .scratch/xdg-portals/niri-integration-review/.
Open result.json, review.json, core.diff/full.diff and install-files.json.
Core selects Settings/FileChooser, retains stock Niri and existing Shell/Notify.
It needs the companion frontend for user-local descriptor discovery. Full selects
17interfaces, with Notification owned by Notify; it additionally needs the Niri
companion, updated Shell/Notify/providers and WirePlumber policy. Print remains
unadvertised; default GNOME/GTK and Secret routes are preserved.

Source archive is accepted phase95:210272638bytes/SHA41ee03f86b7a8f15347a9dcb1e5b37b16acc0969f1061fe8223b245f6e3d3d35.
Only10small metadata files were staged; no binaries copied, no Cargo build or SDK
download. Review artifacts are about1.1MB, mostly the exact1195-member inventory
and installation mapping. Core payload297642305bytes, full837129966bytes; this
is the existing development/debug bundle. Files and CLI binaries, FileManager1
activation and existing Shell/Notify unit definitions are outside both plans.

Live read-only inventory: XDG_CURRENT_DESKTOP=niri, distribution config selected,
stock /usr/bin/niri26.04(8ed0da4), ScreenCastVersion4 without Canopa owner contract.
GNOME/GTK portal processes remain active; Canopa backend/Notify portal absent.
Existing Shell/Notify run older local binaries; Wallpaper portal absent. Preserve
their custom unit definitions and After=niri.service drop-ins.

All review checks PASS and all handles terminal:
- Bundled-checksum-matched Niri, Shell, Notify and Files validate the actual user
  configuration unchanged;11KDL hashes retained. No migration required by checks.
- Private frontend uses actual core candidate, desktop niri and user-local
  descriptors with XDG_DESKTOP_PORTAL_DIR unset. Both selected routes are logged,
  public Settings matches the backend and FileChooserv4 is available. Backend was
  started explicitly; installed systemd activation was not exercised.
- Combined base/unit drop-ins pass systemd-analyze verify with future executable
  paths substituted to checksum-matched builds. Initial sandbox socket failure
  was a verifier restriction; identical host read-only check passes.
- Final live owner/PID and config comparisons unchanged; all3private processes
  exited and temporary runtime removed. No new user overrides/drop-ins installed.

One discovery probe used Shell --help, which is not an early-exit command and
attempted UI startup in the sandbox before NoCompositor panic. No user mutation;
subsequent checks use source-declared --check-config/check-config entrypoints.
Do not repeat that probe or turn it into an unrelated UI work item.

The full profile must be activated between Niri sessions, after required owner
version1/process identity checks. Do not assume all user services restart on
logout: explicitly reload updated Shell/Notify and portal processes at the
appropriate approved point. Never restart/logout the live compositor automatically.

NEXT: user review/approval. All authorized preparation is complete. Do not run
scripts/update.sh (it would build release and restart services), repeat native
matrices, resume Print, or install files merely on automatic goal continuation.
If the user approves the CORE profile, apply only its exact install-files mapping
with archive/input guards, one backup and atomic writes. Do not apply FULL or
restart services under a core install-without-restart approval. Writable-root
escalation and explicit installation authorization are required for ~/.local and
~/.config. Review-only authorization is not installation authorization.
