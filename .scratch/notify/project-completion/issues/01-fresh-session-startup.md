# Verify startup after a fresh graphical login

Status: ready-for-human

Both installed services are enabled, wanted by `graphical-session.target`, and
passed service restart and live checks on 2026-09-21. No logout was forced during
the user's working session.

At the next normal logout/login, confirm `canopa-notify.service` and
`niribar.service` become active, Notify owns `org.freedesktop.Notifications`,
Dunst stays masked, and the labeled Notifications button opens the Center.
Send a normal application notification and open a notification body link. Record
the fresh-session result in `docs/notify/project-completion-verification.md`.

Backups: `~/.local/state/canopa-rollout/20260921-213137`.

## Comments

2026-10-23: The user's reboot exposed a compositor-readiness race in Notify and
stale installed Shell/Notify binaries that reject the new shared `files` block.
Matching optimized binaries restore the live session without changing its config. Both
companion units now wait for `niri.service` readiness, with a regression for that
ordering and cycle verification. See the reboot recovery record in
`docs/notify/project-completion-verification.md`. The next normal reboot still
needs to confirm the corrected startup; no forced logout was performed.
