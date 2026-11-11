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

## Network, sound and media cards

shell-network-wifi = Wi-Fi
shell-network-wired = Wired
shell-network-vpn = VPN
shell-network-network = Network
# Connectivity, shown as the network {label} when no connection name is known.
shell-network-online = online
shell-network-limited = limited
shell-network-offline = offline
shell-network-unknown = unknown
shell-network-connected = connected
shell-network-portal = sign-in required
# $vpn lists the active VPN names.
shell-network-with-vpn = { $description } · VPN: { $vpn }
shell-network-status-unavailable = Network status unavailable
shell-volume-sound = Sound
shell-volume-muted = Output muted
shell-volume-devices = Speakers and microphones
shell-volume-unavailable = Audio status unavailable
shell-volume-level = Output level
# $level is a formatted percentage.
shell-volume-level-value = Output level, { $level }
shell-volume-decrease = Decrease volume
shell-volume-increase = Increase volume
shell-media-title = Media
# Default tooltips: keep the variables; they become {value} and {state}.
shell-tooltip-battery = { $value }% { $state }
shell-tooltip-volume = { $value }%

## Bar and panel chrome

shell-back = Back
shell-close = Close
shell-search = Search
# Accessible window names.
shell-bar-name = Canopa bar
shell-bar-name-on-output = Canopa bar on { $output }
shell-tray-menu = Tray menu
# Caption above the workspace buttons on wide bars.
shell-workspace-label = Workspace
shell-config-alert = Configuration error
shell-config-alert-description = The Shell configuration was rejected
shell-list-use-custom-value = Use custom value
# $value is the text the person typed.
shell-list-use-custom-value-named = Use custom value { $value }
shell-provider-failed = Provider failed
shell-provider-value-too-long = Value is too long. Shorten it and try again.
# A provider's selected entry; $label is its text.
shell-provider-selected = Selected: { $label }

## Bar accessibility (names read by screen readers)

shell-a11y-clock = Clock
shell-a11y-battery = Battery
shell-a11y-battery-full = fully charged
shell-a11y-battery-not-charging = not charging
shell-a11y-battery-charging = charging
shell-a11y-battery-discharging = discharging
shell-a11y-battery-critical = critical
shell-a11y-battery-low = low
shell-a11y-network = Network
shell-a11y-volume = Volume
shell-a11y-volume-muted = muted
shell-a11y-notifications = Notifications
shell-a11y-unavailable = unavailable
shell-a11y-attention = { $count ->
        [0] none need attention
        [one] { $count } needs attention
       *[other] { $count } need attention
    }
shell-a11y-dnd-on = Do Not Disturb on
# $label is the workspace's configured label or number.
shell-a11y-workspace = Workspace { $label }
shell-a11y-workspace-focused = focused
shell-a11y-workspace-active = active
shell-a11y-workspace-urgent = urgent
shell-a11y-tray-item = Tray item
shell-a11y-tray = Tray
shell-a11y-image = Image

## Notices

shell-notice-open-file = Open file
shell-notice-open-settings = Open Settings
shell-config-notice-summary = Shell configuration error
shell-config-notice-defaults = Canopa Shell started with its built-in layout. Fix the file and save it to apply your configuration.
shell-config-notice-previous = Canopa Shell kept the last working configuration. Fix the file and save it to apply your changes.

## Session actions (suspend, hibernate, reboot, power off)

# $action is suspend, hibernate, reboot or power-off.
shell-session-failed = { $action ->
        [suspend] Couldn't suspend
        [hibernate] Couldn't hibernate
        [reboot] Couldn't restart
       *[power-off] Couldn't power off
    }
shell-session-not-authorized = { $action ->
        [suspend] You are not allowed to suspend now, or authentication was cancelled.
        [hibernate] You are not allowed to hibernate now, or authentication was cancelled.
        [reboot] You are not allowed to restart now, or authentication was cancelled.
       *[power-off] You are not allowed to power off now, or authentication was cancelled.
    }
shell-session-logind-unavailable = The login manager (systemd-logind) is not available on the system bus.
# $blockers lists the applications ("Firefox: Downloading files; Steam"); $count is how many.
shell-session-blocked = { $count ->
        [one] Blocked by { $blockers }. Finish or close that application, then try again.
       *[other] Blocked by { $blockers }. Finish or close those applications, then try again.
    }
# Shown under a confirmation's prompt.
shell-session-blocked-by = Blocked by { $blockers }.
shell-session-timed-out = The login manager did not answer in time.
# The last entry of a long list of blocking applications.
shell-session-more-blockers = { $count } more

## On-screen display

shell-osd-name = On-screen display
shell-osd-volume = Volume
shell-osd-microphone = Microphone
shell-osd-brightness = Brightness
shell-osd-keyboard-brightness = Keyboard brightness
shell-osd-muted = Muted
shell-osd-off = Off
shell-osd-unavailable = Unavailable
# Screen reader announcements: "Volume 45%, Speakers".
shell-osd-announce-muted = muted
shell-osd-announce-off = off
shell-osd-announce-unavailable = unavailable
shell-osd-announcement = { $channel } { $value }
shell-osd-announcement-device = { $channel } { $value }, { $device }

## Sound provider

shell-audio-title = Sound
shell-audio-unavailable = Audio is unavailable. Reconnecting…
shell-audio-outputs = Speakers and headphones
shell-audio-no-outputs = No output devices are available.
shell-audio-inputs = Microphones
shell-audio-no-inputs = No input devices are available.
shell-audio-output-volume = Output volume
shell-audio-set-output = Set as output
shell-audio-default-output = Default output
shell-audio-input-level = Input level
shell-audio-set-input = Set as input
shell-audio-default-input = Default input
shell-audio-mute = Mute
shell-audio-unmute = Unmute
shell-audio-muted = Muted
shell-audio-applying = Applying change…
shell-audio-muted-applying = Muted · Applying change…
shell-audio-too-many-changes = Too many audio changes are pending. Please try again.

## Media provider

shell-media-now-playing = Now playing
# The media player choice that follows the active player.
shell-media-automatic = Automatic
shell-media-player = Player
# $status is shell-media-playing, -paused or -stopped.
shell-media-player-option = { $player } · { $status }
shell-media-playing = Playing
shell-media-paused = Paused
shell-media-stopped = Stopped
shell-media-play = Play
shell-media-pause = Pause
shell-media-previous = Previous
shell-media-next = Next
shell-media-no-players = No media players are open.
shell-media-unavailable = Media is unavailable. Reconnecting…
shell-media-timed-out-retry = Media control timed out. Please try again.
shell-media-busy = Media controls are busy
shell-media-timed-out = Media control timed out
shell-media-disconnected = Media controls disconnected
shell-media-player-gone = Player is no longer available
shell-media-expired = Media request expired; try again

## Wi-Fi provider

shell-wifi-title = Wi-Fi
shell-wifi-unavailable = Wi-Fi unavailable
shell-wifi-check-again = Check again
shell-wifi-set-up-in-settings = Set up in Settings
shell-wifi-enterprise-in-settings = Join WPA-Enterprise networks in Settings, on the Network page.
shell-wifi-credential-required = Credential required
shell-wifi-credential-rejected = Credential rejected
shell-wifi-off = Wi-Fi is off
shell-wifi-turn-on = Turn Wi-Fi on
shell-wifi-turn-off = Turn Wi-Fi off
# $network is the network name.
shell-wifi-connecting = Connecting to { $network }
shell-wifi-connected = Connected to { $network }
shell-wifi-scanning-networks = Scanning for Wi-Fi networks
shell-wifi-choose = Choose a Wi-Fi network
shell-wifi-disconnect = Disconnect
shell-wifi-forget = Forget { $network }
shell-wifi-problem = Wi-Fi problem
shell-wifi-credential-title = Network Credential
shell-wifi-credential-prompt = Enter credential for { $network }
shell-wifi-credential-label = Network credential
shell-wifi-connect = Connect
shell-wifi-scanning = Scanning
shell-wifi-refresh = Refresh
shell-wifi-error-target-changed = The selected network changed. Select it again.
shell-wifi-error-no-control = Check that NetworkManager or iwd is running, then try again.
shell-wifi-error-no-backend = No supported Wi-Fi control backend is available.
shell-wifi-error-scan-failed = Wi-Fi scan failed.
shell-wifi-error-connection-failed = Connection failed.
shell-wifi-error-credential-rejected = Network credential was rejected.
shell-wifi-error-unsupported = This Wi-Fi network is not supported.
shell-wifi-error-authorization-required = Wi-Fi authorization is required.
# A network row's description: $known is yes or no, $security is open,
# secured or enterprise, $signal a formatted percentage.
shell-wifi-network-description = { $known ->
        [yes] known { $security ->
            [open] open
            [enterprise] enterprise
           *[secured] secured
        } { $signal }
       *[no] { $security ->
            [open] open
            [enterprise] enterprise
           *[secured] secured
        } { $signal }
    }

## Bluetooth panel

shell-bluetooth-title = Bluetooth
shell-bluetooth-no-adapters = No Bluetooth adapters found.
shell-bluetooth-unavailable = Bluetooth service is unavailable.
shell-bluetooth-searching = Searching for devices…
shell-bluetooth-turn-on = Turn on
shell-bluetooth-turn-off = Turn off
shell-bluetooth-find-devices = Find devices
shell-bluetooth-stop-searching = Stop searching
shell-bluetooth-connected = Connected
shell-bluetooth-paired = Paired
shell-bluetooth-available = Available
shell-bluetooth-connect = Connect
shell-bluetooth-disconnect = Disconnect
shell-bluetooth-pair = Pair
# $code is a six-digit code.
shell-bluetooth-confirm-code = Does the other device show { $code }?
shell-bluetooth-authorize = Allow this pairing request?
shell-bluetooth-service = Allow this device to connect?
shell-bluetooth-enter-pin = Enter the PIN shown by the other device.
shell-bluetooth-enter-passkey = Enter the six-digit passkey shown by the other device.
shell-bluetooth-type-pin = Type { $pin } on the other device and press Enter.
# $entered is how many of the 6 digits were typed.
shell-bluetooth-type-passkey = Type { $passkey } on the other device and press Enter ({ $entered } of 6 digits entered).
# $device is the device name.
shell-bluetooth-pair-with = Pair with { $device }
shell-bluetooth-pairing-code = Pairing code
shell-bluetooth-invalid-code = Enter a valid pairing code.
shell-bluetooth-waiting = Waiting for the device…
shell-bluetooth-cancel-pairing = Cancel pairing
shell-bluetooth-confirm = Confirm
shell-bluetooth-pairing-canceled = Pairing canceled.
shell-bluetooth-change-failed = Bluetooth could not apply the change.
shell-bluetooth-pairing-failed = Bluetooth pairing or connection failed.

## Power and brightness panel

shell-power-title = Power & brightness
shell-power-built-in-display = Built-in display
shell-power-brightness-unavailable = Display brightness is unavailable in this session.
shell-power-profile = Power profile
# $profile is one of the profile names below.
shell-power-profile-active = Active: { $profile }
shell-power-saver = Power saver
shell-power-balanced = Balanced
shell-power-performance = Performance
shell-power-profiles-unavailable = Power profiles service is unavailable.
shell-power-change-failed = The system could not apply the change.
