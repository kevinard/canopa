# Follow local commits on your desktop

This opt-in Linux setup replaces manual binary copies with a user timer. It
follows the local `main` branch of this repository, builds committed source,
installs both executables, and restarts an already-running Niribar service.
It does not fetch or pull remote changes or deploy uncommitted edits. Branch
switches in your working checkout do not change the configured tracking branch.

Requirements: Python 3.11+, Git, Cargo/Rust and the native build dependencies,
systemd user services, Bash and GNU coreutils. The setup expects an existing
`~/.local/bin/niribar` and `niribar.service`, as on the author's desktop.

From the repository:

```sh
python3 scripts/niribar-dev.py install --repository "$PWD" --seed-cache
```

The optional `--seed-cache` copies the existing `target/release` cache once to
avoid rebuilding every dependency. It uses a reflink when the filesystem
supports it, otherwise a normal copy. The updater's target directory is
separate from manual builds. Expect some extra disk use and several minutes
for optimized Rust builds. Checks run once per minute after the previous
update job finishes; this is automatic deployment after compilation, not
instant hot reload.

Installation writes:

- `~/.config/niribar-dev/config.json`: repository, branch, paths and tool locations.
- `~/.local/share/niribar-dev/`: updater, current/previous releases and original backups.
- `~/.cache/niribar-dev/`: a detached Git worktree and dedicated build cache.
- `~/.local/state/niribar-dev/`: update status and the concurrency lock.
- `~/.config/systemd/user/niribar-dev-update.{service,timer}` and a
  `niribar.service.d/50-development.conf` drop-in. Other service settings remain.
- `~/.local/bin/niribar`, `niribar-wifi-provider` and `niribar-dev` symlinks.

The corresponding XDG config/data/cache/state variables are respected. The
timer starts with `graphical-session.target`. It does not start a bar that the
user has stopped. Build output goes to the user journal.

## Update and recovery behavior

The original installation keeps running during compilation. A successful build
must pass the release smoke test, including the Wi-Fi provider handshake and
Close, before activation. Both executables live in one immutable release
directory. Their shared `current` symlink changes atomically, so the bar's
sibling provider comes from the same build.

After restarting an active bar, the updater checks its process identity and
service state for five seconds. An initial failure restores the previous build
and restarts it. This checks initial process health; it cannot detect every
later UI or behavioral regression. The original binaries are retained under
`original/`, and only the current and previous managed releases are retained
after successful updates.

Stopping an update during activation rolls it back. If the updater is forcibly
killed, its activation journal lets the next run restore the previous release
before doing anything else. Recovery respects an explicitly stopped bar.
Moving the tracked branch back to the previous successful commit reuses that
release after checking its binary hashes.

A newer commit arriving during compilation discards that activation attempt;
the next timer tick builds the newest commit. A failed revision is retried
after 15 minutes, or immediately when a new commit arrives. A manual retry can
bypass that cooldown. The user's working checkout is never reset or cleaned.
The updater exclusively owns its detached cache checkout and removes untracked
and ignored files there before each build, after verifying the worktree's
registration. Do not use that cache checkout for development.

## Inspect or control the installation

```sh
niribar-dev status
journalctl --user -u niribar-dev-update.service -n 60
systemctl --user list-timers niribar-dev-update.timer
niribar-dev update --retry
```

Status includes the desired and installed Git revisions, binary hashes, last
update outcome and whether the running process is the installed executable.
The installed application still reports the Cargo package version through
`niribar --version`; the updater tracks the exact revision independently.

To stop automatic updates while keeping the current installed version:

```sh
systemctl --user disable --now niribar-dev-update.timer
systemctl --user stop niribar-dev-update.service
```

To restore the installation from before this setup, first stop the timer and
update service, then stop Niribar, remove the managed bin symlinks and the
`50-development.conf` drop-in, copy the saved `original/` binaries back into
`~/.local/bin`, reload the user manager and start Niribar again. Preserve any
other service drop-ins. The original installation may not have included the
Wi-Fi helper. Remove the updater's managed worktree with `git worktree remove`
before deleting its cache directory.

Rerun `install` from the project when intentionally upgrading the updater
itself or changing its configuration; ordinary application commits need no
installation command. No system service or root installation is required.
