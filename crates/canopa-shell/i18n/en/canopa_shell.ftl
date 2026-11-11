# Canopa Shell: bar, panels and launcher. English is the source language.

## Panel titles

# Heading of the bar panel when the configuration gives no title.
shell-panel-overview = Overview
shell-panel-quick-settings = Quick settings
shell-panel-calendar = Calendar
shell-panel-sound = Sound
shell-panel-network = Network
shell-panel-notifications = Notifications
shell-panel-idle-inhibitor = Idle inhibitor
shell-panel-battery = Battery
shell-panel-workspaces = Workspaces
shell-panel-focused-window = Focused window
shell-panel-launcher = Launcher
# Heading of a `canopa-shell dmenu` list.
shell-panel-choose-item = Choose an item
shell-panel-wifi = Wi-Fi

## Built-in bar buttons (config/default.kdl `label-id` and `tooltip-id`)

shell-module-search = Search
shell-module-overview = Overview
shell-module-controls = Controls
shell-module-bluetooth = Bluetooth
shell-module-power-and-display = Power & display
shell-module-lock = Lock
shell-module-suspend = Suspend
shell-module-hibernate = Hibernate
shell-module-log-out = Log out
shell-module-reboot = Reboot
shell-module-power-off = Power off
shell-tooltip-launcher = Launcher
shell-tooltip-overview = Overview
shell-tooltip-quick-settings = Quick settings
# Workspace button tooltip; $index is the workspace number.
shell-tooltip-workspace = Workspace { $index }

## Built-in menus (config/default.kdl `title-id`, `prompt-id`, `label-id`)

# Placeholder of the launcher's search field.
shell-menu-launcher-prompt = Search apps, files, and more
shell-menu-log-out-title = Log out?
shell-menu-reboot-title = Reboot?
shell-menu-power-off-title = Power off?
# Shown in the log out, reboot and power off confirmations.
shell-menu-unsaved-work = Unsaved work may be lost.
shell-menu-cancel = Cancel
shell-menu-log-out = Log out
shell-menu-reboot = Reboot
shell-menu-power-off = Power off

## Calendar

shell-calendar-today = Today
# Accessible name of today's cell; $date is the full date.
shell-calendar-today-cell = Today, { $date }
shell-calendar-previous-month = Previous month
shell-calendar-next-month = Next month

## Shared control words

# Switch state shown beside a toggle and as the {value} of on/off modules.
shell-switch-on = On
shell-switch-off = Off
shell-status-unavailable = Unavailable
shell-action-retry = Retry
# Accessible description of a control with a trailing status word.
shell-control-description-status = { $description }, { $status }

## Notifications module

shell-notifications-do-not-disturb = Do Not Disturb
# $time is a time of day, possibly with a weekday (see shell-dnd-weekday-time).
shell-dnd-until = Until { $time }
shell-dnd-until-inline = until { $time }
shell-dnd-scheduled-until = Scheduled until { $time }
shell-dnd-scheduled-until-inline = scheduled until { $time }
shell-dnd-until-turned-off = Until turned off
shell-dnd-until-turned-off-inline = until turned off
# A time on another day: $weekday is abbreviated ("Thu").
shell-dnd-weekday-time = { $weekday } { $time }
shell-notifications-fullscreen = Fullscreen: banners are saved quietly
shell-notifications-screen-sharing = Screen sharing: banners are hidden
# Tooltip parts, joined with " · ".
shell-notifications-attention = { $count } require attention
shell-notifications-dnd-off = DND off
# $detail is shell-dnd-until-inline or similar.
shell-notifications-dnd-on-detail = DND on ({ $detail })
shell-notifications-click-help = Click to open · Right-click to toggle DND
shell-notifications-unavailable-reconnecting = Notifications unavailable · Reconnecting
# Bar label while Do Not Disturb is on; $count is the formatted number.
shell-notifications-count-dnd = { $count } · DND
shell-notifications-unavailable = Notifications unavailable
# $detail is shell-dnd-until or similar.
shell-notifications-banners-paused = Banners are paused · { $detail }
shell-notifications-banners-allowed = Banners are allowed

## Idle inhibitor module

shell-idle-help-off = Idle inhibitor off · Click to keep awake
shell-idle-help-on = Idle inhibitor on · Click to allow idle
shell-idle-help-pending = Updating idle inhibitor…
shell-idle-help-unavailable = Idle inhibition unavailable · Requires a visible bar and compositor support
# Stasis paused the session idle on its own (for example for media).
shell-idle-inhibited = Inhibited
shell-idle-keep-awake = Keep awake
shell-idle-paused = Screen idle is paused
shell-idle-automatic = Screen can idle automatically
shell-idle-updating = Updating…
shell-idle-stasis-unavailable = Stasis unavailable
shell-idle-compositor-unavailable = Unavailable on this compositor
# Shown while the inhibitor is changing.
shell-idle-wait = Wait
# $state and $profile come from the Stasis daemon; $hold is "on" or "off".
shell-idle-stasis-help = Stasis: { $state } · Profile: { $profile } · Manual hold: { $hold ->
        [on] on
       *[off] off
    } · Click to toggle manual hold
shell-idle-stasis-help-unavailable = Stasis unavailable · Start canopa-idle and install its backend
