# Distribution-default readiness

Status: needs-triage

Goal: make Canopa credible as the default desktop of a new Linux distribution:
a coherent, flexible and powerful environment that uses Rust components wherever
a good one exists. Canopa does not need to write everything itself. It owns
presentation, coherence and the glue between system services. Existing services
and applications that do their job well are adopted.

Baseline: the 2026-11-10 deep review (security, packaging and forks, feature
coverage, code health). The desktop already runs daily on the maintainer's
machine. That proves the core loop, not the coverage below.

## Principles

1. **System services own state; Canopa owns presentation.** This continues the
   existing ADR rule that "existing owners retain policy and lifetime". Settings
   talks to NetworkManager, BlueZ, systemd-timedated/localed/hostnamed,
   AccountsService, CUPS, UDisks, UPower, power-profiles-daemon, fwupd and
   PackageKit/Flatpak over D-Bus with zbus. Canopa adds no private daemons for
   state those services already keep.
2. **Order of preference** for each missing capability:
   1. an existing service with a stable API, plus a Canopa Rust/Iced frontend;
   2. an existing Rust application that fits;
   3. an existing non-Rust application, when no Rust option is good enough;
   4. a new Canopa tool, only when the result has to feel native (Settings
      pages, Shell surfaces, first-run setup) or nothing suitable exists.
3. **Coherence through shared contracts, not a monolith.** Third-party apps
   follow Canopa's appearance through the Settings portal: color scheme, accent,
   contrast and reduced motion. Canopa adds theme bridges where portals are not
   enough (GTK, Qt, COSMIC). Every Canopa UI uses `canopa-ui` and `canopa-theme`.
4. **Run on stock upstreams by default.** Forked behaviour must be optional and
   must degrade gracefully, and every fork needs a recorded upstreaming path.
5. **Test what ships.** Release profile, packages and compositor build all run
   under CI or a recorded hardware matrix.

## Phase 0: review fixes (done 2026-11-10)

- [x] Helpers survive package upgrades. `canopa_file::executable::current()`
  re-executes the running image instead of a replaced `(deleted)` path. It is
  used by the lock listener, PAM worker, greeter helpers, polkit identity helper,
  portal dialogs and Settings guards. External programs get
  `executable::installed()`. A unit test simulates an upgrade.
- [x] An invalid lockscreen configuration falls back to defaults in both the
  listener and the locker. Locking never becomes a no-op.
- [x] A locker that dies while the session is locked is relaunched: 750 ms delay,
  at most 5 times a minute.
- [x] Transient logind or subscription errors reconnect inside the listener
  instead of ending the service.
- [x] Release builds unwind (`panic = "unwind"`), so the 40 `catch_unwind`
  recovery paths in Files, Notify and the portal work in shipped builds.
- [x] License metadata: the `canopa` package declares GPL-3.0-or-later, because
  Shell and Settings link `niri-ipc`. `LICENSES.md` and the vendor documentation
  now name the shipped `niri-remote-touch` compositor recipe.
- [x] ADR 0120 matches the implemented lock behaviour.

Still open from the review (moved into the workstreams below):
- polkit dialog spoofing (W7);
- the stock `niri` package dependency and the `niri msg` logout command (W1);
- the Debian PAM file is never installed (W1);
- build-time network access and `--nodeps` builds (W1).

## Workstreams

### W1. Foundation: packaging, compositor and release engineering

| Item | Approach | Exit criterion |
| --- | --- | --- |
| Clean builds | Fetch every source in `prepare()` (`cargo fetch --locked` for workspace and Niri); build offline; run `makechrootpkg` in CI | Clean-chroot package build passes in CI with no network in `build()` |
| Package repository | Signed pacman repository built by CI from tags | `pacman -Syu` upgrades a test VM between two tagged versions |
| One compositor | Drop the stock `niri` runtime dependency. Shell logout and Stasis call the private compositor, or better, send `Action::Quit` through `niri-ipc` directly | No Canopa code path runs `niri` from PATH |
| Compositor in CI | Build the shipped overlay in CI; run `desktop.yml` against that exact executable on each release candidate | The release candidate's acceptance artifacts name the shipped compositor's checksum |
| Hardware matrix | Recorded checklist on 2–3 physical machines (the maintainer's daily machine first): boot→greeter→session, VT switch, suspend/resume while locked, lid, hotplug monitors, touchpad, upgrade while locked | A checklist file per release under `.scratch/hardware/` |
| Versioning | Public repository, semver tags, changelog, `schema-version` promise for `config.kdl` with migration through `canopa config` | v0.2.0 tagged; config compatibility policy documented |
| Distro-neutral paths | Detect `/usr/libexec` vs `/usr/lib` for the portal frontend; install the Debian PAM file on Debian | Staging install passes for Arch and Debian layouts |

### W2. Upstream forks

Each fork gets an upstream issue or PR link, an owner and a fallback.

- **Niri, 11k-line overlay.** Split the overlay into:
  - (a) features upstream would accept, such as drag-source lifetime fixes and
    screencast resume: submit PRs;
  - (b) desktop-sharing services (InputCapture, GlobalShortcuts, RemoteInput
    and touch leases): discuss with upstream whether they belong in Niri,
    preferably as Wayland protocols or the D-Bus interfaces Niri already uses
    for screencasting;
  - (c) everything else: drop it.

  The target is a Canopa session that runs on stock Niri, with screen sharing
  through upstream mechanisms and the advanced portals as an optional add-on.
- **xdg-desktop-portal, 4 patches, about 460 lines.** Submit upstream. The input
  ordering patch is the main candidate.
- **winit and Iced, about 5.4k lines.** Track Iced's move to winit 0.31 and port
  before it lands. Upstream drag-and-drop, keyboard-shortcut inhibition and
  xdg-foreign, coordinating with the COSMIC fork, which has similar work.
- **CUPS client and backend.** Keep them only while Print is opt-in. If Print
  becomes default, use upstream libcups3 or a PAPPL-based path instead of a
  private libcups.

### W3. Settings coverage

New Settings pages are Rust/Iced frontends over the system services:

| Page | Service | Notes |
| --- | --- | --- |
| Network | NetworkManager (zbus proxies; COSMIC's NM crate is a reference) | Wired, VPN (WireGuard built in, OpenVPN/IPsec plugins, `.ovpn`/`.conf` import), 802.1X/WPA-Enterprise, hotspot, proxy, metered. Shell quick settings keep their current role; iwd stays the alternative backend |
| Bluetooth | BlueZ via the `bluer` crate | Promote from quick settings to a full page; pairing history, battery, audio profile |
| Sound | PipeWire and WirePlumber (`pipewire-rs` or the existing pulse binding) | Per-application volume, card profiles, input level meter, alert sound |
| Power | UPower, power-profiles-daemon, logind | Battery health, lid and power-button actions; adopt Stasis (Rust) as a hard dependency, or absorb its policy into `canopa-idle` |
| Brightness and media keys | logind `SetBrightness` (no root), Shell OSD | XF86 key bindings in defaults plus Shell OSD for volume, brightness and keyboard backlight |
| Night light | Niri's gamma-control support (validate) | Shell toggle and schedule; `wl-gammarelay-rs` (Rust) is a fallback |
| Date and time | systemd-timedated | Time zone with search and automatic zone via geoclue, NTP, 12/24 h format |
| Region and language | systemd-localed plus the W5 locale switch | System language, formats, keyboard defaults |
| Users | AccountsService | Add, remove and edit users, avatar (reusing greeter avatar rules), administrator flag, password through polkit |
| Default applications | `canopa-apps` + `mimeapps.list` | Browser, mail, files, terminal, viewers; replaces the `xdg-mime` instructions |
| Printers | CUPS through IPP (cups-pk-helper or direct IPP with polkit) | Driverless IPP Everywhere discovery, queue management, jobs. Separate from the Print portal decision |
| Disks and removable media | UDisks | Mount policy, format and encrypt removable drives (or recommend GNOME Disks) |
| Privacy | Lockdown portal, portal permission store | Per-app portal grants, location, camera/microphone, screen-sharing history |
| About and updates | hostnamed, os-release, fwupd, W6 | Hostname, hardware, firmware updates |

Exit criterion: everything a new user needs can be configured without a terminal,
on a laptop with Wi-Fi, wired, VPN, Bluetooth audio and a printer.

### W4. Applications

**Decision 2026-11-11: out of scope.** Users choose their own applications; Canopa
ships no default app set. Current focus is the core experience: Shell, launcher,
Settings, Files and Notify. The table below is kept only as reference for users
and for the portal theme bridges (contrast and reduced motion through the
Settings portal), which still matter so that any app a user picks follows
Canopa's appearance.

Canopa keeps building the shell, Files, Settings, launcher, notifications,
lock, greeter and agents. For everything else it adopts existing apps, Rust
first:

| Role | Default candidate | Rust | Alternative |
| --- | --- | --- | --- |
| Terminal | Alacritty | yes | COSMIC Terminal (Iced-based); Foot stays available |
| Text editor | COSMIC Text Editor | yes | Helix (TUI), Zed (heavy) |
| Image viewer | Loupe (GTK4, glycin decoders) | yes | Files Quick Look for previews |
| Document viewer | Papers (GNOME, partially Rust) | partly | Zathura |
| System monitor | Mission Center or Resources | yes | `btop` |
| Music | Amberol | yes | — |
| Video | Clapper or Showtime | no | mpv |
| Software and updates | COSMIC Store (Flatpak + PackageKit/AppStream) | yes | own PackageKit/Flatpak frontend if theming or scope don't fit |
| Archives | Files actions over `libarchive` / `compress-tools`, or File Roller | partly | `ouch` (Rust CLI) |
| Screenshots | Existing portal + Niri action; add Shell annotation later | yes | Satty (Rust) for annotation |
| Calculator | Launcher (libqalculate) | — | — |
| Backups | Pika Backup (optional) | yes | — |

Coherence follow-ups:
- Publish contrast and reduced-motion through the Settings portal; it currently
  publishes only color scheme and accent.
- Add theme bridges: libadwaita accent and color scheme through the portal, a
  generated GTK3/Qt palette, and a Canopa→COSMIC theme exporter if COSMIC apps
  are adopted. libcosmic ships its own Iced fork and theme, which is a coherence
  cost to weigh against COSMIC apps being Rust and Iced-like.

Exit criterion: a fresh install has a full default app set that follows the
Canopa light/dark theme and accent.

### W5. Internationalization

Status 2026-11-11: framework, lint, ICU4X formatting, RTL mirroring and a
French translation (machine-assisted, needs review) are in place for every
component ([ADR 0145](../../docs/adr/0145-fluent-translations-and-icu-formatting.md)).
An RTL translation and the input-method work remain open.

- Framework: Project Fluent through `i18n-embed` and `i18n-embed-fl`. These are
  Rust-native, check message IDs at compile time, and are already used by
  COSMIC. Weblate supports Fluent for community translation.
- Order: shared crates (`canopa-ui`, `canopa-session-ui`) → lock, greeter,
  polkit and pinentry → Shell and Notify → Settings → Files → portal dialogs.
  Add a lint that rejects new user-visible string literals outside Fluent.
- Locale-aware formatting: dates and numbers through ICU4X, which is Rust. Today
  clock and date formats are user-configured `chrono` patterns that ignore the
  locale.
- RTL: layout mirroring in `canopa-ui` (Iced alignment), with screenshot tests
  in an RTL locale. Bidi shaping already exists in `canopa-session-ui`.
- Input methods: validate fcitx5 (text-input-v3/input-method-v2 on Niri) in
  every Iced app and layer surface; fix the vendored layershell path if needed.
  Ship a documented CJK setup.

Exit criterion: the whole desktop is fully translated into two languages, one of
them RTL, and Chinese/Japanese input works in the launcher, Files and Settings.

### W6. Accessibility

- Generalize the existing AccessKit integration (Shell, Notify,
  `canopa-session-ui`) into `canopa-ui`, so every Iced app gets an AT-SPI tree.
  Files, Settings, launcher and portal dialogs come first.
- Validate with Orca on real hardware for every release. Track Odilia, the Rust
  screen reader, as a future option.
- Add Settings pages: high-contrast mode, larger text and cursor, reduced motion
  (already present), sticky/slow/bounce keys (through Niri/XKB), a screen
  magnifier (Niri support to investigate), and an on-screen keyboard
  (Squeekboard, Rust, or wvkbd).
- Extend the existing contrast checker (`scripts/check-contrast.py`) into CI for
  every theme.

Exit criterion: a blind user can log in, launch apps, manage files, change
settings and lock or unlock using Orca only.

### W7. Security hardening

- External audit of the authentication stack:
  - the PAM worker (`auth.rs`);
  - `canopa-polkit/native/agent.c`;
  - secret storage;
  - the greetd state machine;
  - the Niri ext-session-lock paths;
  - the `accesskit_unix` fork.
- Polkit dialog spoofing. Options, in order of effort:
  1. show the requesting program's verified identity (executable name from
     `/proc/<pid>/exe` and its package) instead of a bare PID;
  2. present the dialog as a compositor-known privileged surface, such as a
     layer-shell overlay with a dimmed desktop that only Canopa's agent can
     request (Wayland security-context or Niri client identity);
  3. add a secure attention key that only the compositor can deliver.
- Lock-screen accessibility input: restrict AT-SPI actions on the lock surface
  to the screen reader the user enabled.
- Secure boot, systemd-homed or LUKS: document what the distro owns and what
  Canopa owns.

### W8. First-run and installation

- Canopa Setup, a Rust/Iced first-run app in the greeter session or the first
  login. It covers language, keyboard, network, time zone, user account, privacy
  defaults and theme, using the same services as W3.
- The installer is the distribution's choice (Calamares, archinstall or a
  custom one). Canopa provides the post-install setup and a live-session profile.
- Testing: openQA (or a scripted QEMU suite) covering install → first-run →
  login → upgrade.

### W9. Files completeness

- Network locations through GIO/GVFS (SMB, SFTP, WebDAV, MTP). Files already
  uses GIO for devices, so this extends an existing seam instead of adding a
  new stack.
- Thumbnails: the freedesktop thumbnail cache and spec, reusing installed
  thumbnailers (glycin, gdk-pixbuf, ffmpegthumbnailer) in sandboxed workers.
- Content search: integrate an indexer (LocalSearch/Tracker or a Rust
  alternative) through the launcher's provider protocol, not inside Files.
- Preserve ACLs and xattrs on copy. They are dropped today.
- Archive create/extract (see W4).
- Clear the recurring workspace timing failures recorded in
  `.scratch/file-manager/CURRENT-FAILURES.md`, so the workspace gate is green.

### W10. Sustainability

- Contributor onboarding: `CONTRIBUTING.md`, `SECURITY.md`, a one-page
  architecture overview, and per-crate "start here" sections. Condense the 370+
  ADRs into maintained reference docs; keep ADRs as history.
- Scope discipline: keep Print, InputCapture, GlobalShortcuts, USB and
  RemoteDesktop opt-in until their acceptance is complete. Don't let them block
  W3–W6. Consider routing Print through xdg-desktop-portal-gtk for the first
  distro release.
- Recruit at least two co-maintainers for the security stack and Files before
  any distro commits to Canopa as its default.
- Robustness budget: `#![forbid(unsafe_code)]` in crates without FFI; crash
  reporting through systemd-coredump with a "report a problem" path from Notify.

## Current focus (2026-11-11)

The core experience first: Shell, launcher, Settings, Files and Notify. Fork
upstreaming (W2) is paused. W1 foundation items that every component ships
through stay in scope.

## Milestones

| Milestone | Contents | Indicative scope |
| --- | --- | --- |
| M1 Daily-driver complete | W1 clean builds, single compositor, hardware matrix; W3 network, sound, power, keys/OSD, date/time, default apps; W4 app set chosen and packaged | 2–3 months |
| M2 Everyone can use it | W5 framework + 2 languages + IME; W6 Orca across all Canopa apps; W3 users, region, printers, privacy | 3–6 months |
| M3 Distribution profile | W8 Canopa Setup + openQA; W4 software/updates and firmware; W7 audit; W9 network shares and thumbnails | 6–9 months |
| M4 Upstream-first | W2 overlay on stock Niri, portal patches upstreamed, winit 0.31 port; W10 co-maintainers | ongoing; prerequisite for non-Arch distros |

## Decisions needed

1. Base distribution: an Arch-based spin, which matches today's packaging, or a
   different base? This decides package format, installer and update stack.
2. ~~Third-party app family~~ and 3. ~~Flatpak-first or traditional packages~~:
   resolved 2026-11-11. Users choose their apps; no default set (see W4).
4. When does the repository become public, and under which governance?
5. Print in the first distro release: Canopa's opt-in backend, or GTK's?

## Comments
