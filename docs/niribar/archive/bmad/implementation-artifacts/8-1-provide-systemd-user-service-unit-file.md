# Story 8.1: Provide systemd User Service Unit File

Status: review

## Story

As a user who wants niribar to start automatically with my session,
I want a systemd user service unit file shipped with the project,
so that I can enable it with `systemctl --user enable niribar` without writing one myself.

## Acceptance Criteria

1. **Given** the niribar repository
   **When** a user installs the package
   **Then** `packaging/systemd/niribar.service` is present in the repository and available as an installation artifact

2. **Given** the service unit file
   **When** it is loaded by systemd
   **Then** it starts niribar as a user service after `graphical-session.target`

3. **Given** the service unit file
   **When** niribar crashes or exits non-zero
   **Then** systemd restarts the process (Restart=on-failure)

4. **Given** the service unit file
   **When** inspected with `systemctl --user status niribar`
   **Then** the description correctly identifies the binary and project

5. **Given** the repository
   **When** a user reads the default config file (`crates/niribar/config/default.kdl`)
   **Then** a comment explains how to enable the systemd service

## File Map

| File | Action | What changes |
|------|--------|--------------|
| `packaging/systemd/niribar.service` | **WRITE** | Replace placeholder comment with real systemd unit file |
| `crates/niribar/config/default.kdl` | **MODIFY** | Add header comment block explaining systemd service usage |

## Tasks / Subtasks

- [x] Task 1: Write `packaging/systemd/niribar.service` (AC: #1–4)

  Write the following content to `packaging/systemd/niribar.service`, replacing the existing placeholder comment:

  ```ini
  [Unit]
  Description=niribar status bar
  After=graphical-session.target
  PartOf=graphical-session.target

  [Service]
  ExecStart=/usr/bin/niribar
  Restart=on-failure
  RestartSec=3

  [Install]
  WantedBy=graphical-session.target
  ```

- [x] Task 2: Add documentation comment to `crates/niribar/config/default.kdl` (AC: #5)

  Prepend the following comment block to the top of `crates/niribar/config/default.kdl`, before the existing content:

  ```kdl
  // To start niribar automatically with your Wayland session:
  //
  //   # Copy the service file (if not installed by your package manager)
  //   cp packaging/systemd/niribar.service ~/.config/systemd/user/
  //
  //   # Enable and start the service
  //   systemctl --user enable --now niribar.service
  //
  // The service starts after graphical-session.target and restarts on failure.
  //
  ```

## Dev Notes

### This story has NO Rust code changes

This story is pure infrastructure — two file edits only. Do not touch any `.rs` files.

### `packaging/systemd/niribar.service` — the placeholder

The file at `packaging/systemd/niribar.service` currently contains only a placeholder comment:
```
# placeholder — Story 8.1 populates the systemd user service unit here
```
Replace its entire contents with the unit file in Task 1.

### Why `PartOf=graphical-session.target`

`PartOf=` means: if `graphical-session.target` stops or restarts, niribar stops too. This is the correct lifecycle for a bar — it has no business running without a graphical session. `Wants=` is not used because niribar being enabled should not attempt to start the graphical session.

### Why no `Environment=` directives

Niri (the compositor) uses `niri --session` which calls `systemctl --user import-environment` to export Wayland environment variables (`WAYLAND_DISPLAY`, `NIRI_SOCKET`, `XDG_CURRENT_DESKTOP`, `DBUS_SESSION_BUS_ADDRESS`, etc.) into the systemd user session before `graphical-session.target` is reached. Services starting `After=graphical-session.target` automatically inherit these variables — no explicit `Environment=` lines needed.

### `ExecStart=/usr/bin/niribar` — packaged install path

The service targets packaged installs where the binary is at `/usr/bin/niribar` (standard distro convention). Users doing dev builds (e.g., `cargo install`) can copy the service to `~/.config/systemd/user/` and override `ExecStart` via a drop-in. Do NOT use `%h/.cargo/bin/niribar` — contrib service files target proper package installs.

### `RestartSec=3` rationale

A 3-second restart delay prevents a fast crash loop from spamming the compositor. This is the widely-used default for GUI application services (see Waybar, Swaybar, etc.).

### `crates/niribar/config/default.kdl` — existing header

The file currently begins with:
```kdl
// niribar default configuration
// Override by placing your config at $XDG_CONFIG_HOME/niribar/config.kdl
```
Prepend the new systemd comment block BEFORE this existing header, not after. The existing header must remain intact.

### No cargo/Rust changes → no test baseline change

`cargo test` count remains 377. No new tests are required for this story — there is no Rust code to test.

### Project Structure Notes

Architecture canonical location for service file:
```
contrib/
└── systemd/
    └── niribar.service     # FR40 — systemd user service unit
```
[Source: architecture.md, "Canonical Directory Tree"]

Config file location (for documentation context):
```
config/
└── default.kdl             # fallback config, embedded in binary (FR42, Story 8.3)
```

### References

- [Source: epics.md, Epic 8, Story 8.1 — acceptance criteria]
- [Source: architecture.md, FR39–43 packaging/distribution section]
- [Source: architecture.md, "Artifacts: Compiled binary + crates/niribar/config/default.kdl + packaging/systemd/niribar.service"]

## Dev Agent Record

### Agent Model Used

claude-sonnet-4-6

### Debug Log References

### Completion Notes List

- Wrote `packaging/systemd/niribar.service`: real systemd user unit replacing placeholder; `After=PartOf=graphical-session.target`, `Restart=on-failure`, `RestartSec=3`, `ExecStart=/usr/bin/niribar`, `WantedBy=graphical-session.target`
- Modified `crates/niribar/config/default.kdl`: prepended 9-line comment block with `systemctl --user enable --now niribar.service` instructions before existing header; existing content unchanged
- No Rust code changes; `cargo test` baseline of 377 tests unaffected

### File List

- `packaging/systemd/niribar.service`
- `crates/niribar/config/default.kdl`

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-04-12 | 1.0 | Implemented: systemd user service unit file + default.kdl documentation comment | claude-sonnet-4-6 |
