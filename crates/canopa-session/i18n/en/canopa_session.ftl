# Canopa Session lock and wallpaper. English is the source language.

## Lock screen

# Window title and screen-reader name of the lock screen.
lock-title = Canopa Lock
lock-heading = Unlock session
# Shown below the person's name.
lock-subtitle = Enter your password to unlock
lock-unlock = Unlock
lock-try-again = Try again
lock-starting-authentication = Starting authentication…
lock-timed-out = Authentication timed out. Enter to retry.
lock-could-not-start = Authentication could not start. Enter to retry.
lock-failed = Incorrect password. Try again.
lock-cancelled = Authentication cancelled. Enter to retry.
lock-password-change-required = Password change required
lock-password-expired = Your password has expired. Change it to unlock, or cancel to remain locked.
lock-change-password = Change password
lock-cancel-password-change = Cancel password change
# Buttons with the key that does the same.
lock-change-password-shortcut = Change password · Enter
lock-cancel-shortcut = Cancel · Escape

## Wallpaper picker

wallpaper-title = Canopa Wallpaper
wallpaper-apply = Apply wallpaper
wallpaper-applying = Applying…
wallpaper-close = Close
# The selected image. $name is its file name, $position its place in the
# list of $count images.
wallpaper-selection = Wallpaper  ·  { $name }  ·  { $position } of { $count }
# Monitor choice when the wallpaper applies to every monitor.
wallpaper-all-monitors = All monitors
# Buttons with the keys that do the same.
wallpaper-previous-image = Previous image   · ←
wallpaper-next-image = Next image   · →
# $output is a monitor name or "All monitors".
wallpaper-monitor = Monitor: { $output }   · ↑/↓
# $fit is one of the wallpaper-fit-* names.
wallpaper-fit = Fit: { $fit }   · F
wallpaper-fit-fill = Fill
wallpaper-fit-fit = Fit
wallpaper-fit-stretch = Stretch
wallpaper-fit-center = Center
wallpaper-close-shortcut = Close   · Escape
wallpaper-applying-message = Applying the wallpaper…
wallpaper-preparing-preview = Preparing monitor preview…
wallpaper-preview-hint = Enter applies and saves · Tab selects · F5 refreshes
wallpaper-preview-failed = This image could not be previewed. Choose another image.
wallpaper-applied = Wallpaper applied and saved
wallpaper-apply-failed = Wallpaper change failed; the previous assignment was retained
wallpaper-none-found = No pictures were found in your Pictures folder or the installed wallpapers. Choose a picture file instead.

## canopa-idle
idle-not-installed = { $program } is not installed in PATH
idle-already-running = Canopa idle supervisor is already running for this display
idle-locker-missing = Install canopa-lock beside canopa-idle
idle-coordinator-unavailable = The selected session's Canopa lock and sleep listener is not running; start canopa-lock.service first
idle-suspend-timeout = The suspend request timed out
idle-config-missing = No Stasis configuration; create one with canopa idle init
idle-config-invalid = The Stasis configuration must be a regular file of at most 1 MiB
idle-stasis-exited = Stasis stopped: { $status }
idle-coordinator-stopped = The Canopa lock and sleep listener stopped
