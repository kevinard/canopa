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

## Launcher: scope keywords
# Words that start a scoped search, besides the English ones (apps, files,
# windows, settings, clip, snip, emoji, symbol), which always work. List
# alternatives separated by spaces, lowercase, without punctuation other than "-".

shell-keyword-apps = apps
shell-keyword-files = files
shell-keyword-windows = windows
shell-keyword-settings = settings
shell-keyword-clipboard = clip
shell-keyword-snippets = snip
shell-keyword-emoji = emoji
shell-keyword-symbol = symbol

## Launcher: window, search field and results

shell-launcher-scope-all = All
shell-launcher-scope-apps = Apps
shell-launcher-scope-files = Files
shell-launcher-scope-windows = Windows
shell-launcher-scope-settings = Settings
shell-launcher-search-apps = Search apps
shell-launcher-search-files = Search files
shell-launcher-search-windows = Search windows
shell-launcher-search-settings = Search settings
shell-launcher-close = Close launcher
# $key is the key that closes it (shell-key-escape).
shell-launcher-close-hint = Close launcher · { $key }
shell-launcher-clear = Clear
shell-launcher-clear-search = Clear search
shell-launcher-clear-search-hint = Clear search · { $key }
shell-launcher-calculator = Calculator
# Around the "=" key cap: "Type [=] to calculate".
shell-launcher-calculate-before = Type
shell-launcher-calculate-after = to calculate
shell-launcher-select = Select
shell-launcher-installed-apps = Installed apps
shell-launcher-browse = Browse
shell-launcher-results = Results
shell-launcher-actions = Actions
# $name is the result's name.
shell-launcher-actions-for = Actions for { $name }
shell-launcher-open = Open
shell-launcher-open-named = Open { $name }
shell-launcher-launch = Launch { $name }
shell-launcher-no-matches = No matches
shell-launcher-advice-scope = Try another term or choose All.
shell-launcher-advice-spelling = Check the spelling or try a shorter query.
shell-launcher-match-count = { $count ->
        [one] { $count } match
       *[other] { $count } matches
    }
shell-launcher-match-count-approximate = { $count } matches · { $approximate } approximate
shell-launcher-hint-unpin = Remove this result from launcher suggestions
shell-launcher-hint-pin = Keep this result in launcher suggestions
shell-launcher-hint-alias = Find this result with a custom search term
# Key caps shown beside commands.
shell-key-escape = Esc
shell-key-enter = Enter
shell-key-tab = Tab
# $digit is 1 to 4.
shell-key-ctrl-digit = Ctrl+{ $digit }

## Launcher: sessions, apps and status

shell-launcher-application = Application
shell-launcher-application-action = Application action
shell-launcher-matched-command = Matched app file or command name
shell-launcher-matched-description = Matched app description
shell-launcher-matched-keywords = Matched app keywords
shell-launcher-approximate-match = Approximate keyword match
# $path is the file to open.
shell-launcher-open-with-path = Open with · { $path }
shell-launcher-open-with-file = Open with — { $file }
shell-launcher-recent = Recent
shell-launcher-filter-actions = Filter actions
shell-launcher-filter-applications = Filter applications
shell-launcher-snippet-name = Snippet name
shell-launcher-alias-for = Alias for { $name }
shell-launcher-applying-action = Applying action…
shell-launcher-snippet-name-help = Type a snippet name and press Enter to save
shell-launcher-alias-help = Type a unique single-word alias, then Enter · empty removes the alias
shell-launcher-query-too-long = Search is limited to { $bytes } bytes
shell-launcher-enter-expression = Enter an arithmetic expression
shell-launcher-enter-file-name = Enter a file or folder name
shell-launcher-finding-applications = Finding applications…
shell-launcher-calculating = Calculating…
shell-launcher-top-matches = Showing the highest-ranked app matches.
shell-launcher-indexing-files = Indexing file names…
shell-launcher-searching-files = Searching files…
# $name is an extension's name.
shell-launcher-searching-extension = Searching { $name }…
shell-launcher-application-gone = Application is no longer available
# $name is the application; $error the system's reason.
shell-launcher-could-not-start = Could not start { $name }: { $error }
shell-launcher-action-failed = Could not apply action; check the application or file is available
shell-launcher-more-matches = More matches; refine your search
shell-launcher-query-long = Query is too long
shell-launcher-extension-error = { $name }: { $error }
shell-launcher-preferences-stopped = Launcher preferences worker stopped
shell-launcher-remove-alias = Remove alias
shell-launcher-save-alias = Save alias
shell-launcher-save-alias-named = Save alias: { $alias }
shell-launcher-pinned = Pinned
shell-launcher-shortcut = Shortcut
shell-launcher-pinned-alias = Pinned · { $alias }
shell-launcher-shortcut-alias = Shortcut · { $alias }
shell-launcher-switch-to = Switch to
shell-launcher-new-window = New window
shell-launcher-running-application = Running application
shell-launcher-unpin = Unpin from Launcher
shell-launcher-pin = Pin in Launcher
shell-launcher-set-alias = Set search alias…
shell-launcher-edit-alias = Edit search alias…
# $action is a session action such as "Log out"; it opens a confirmation.
shell-session-confirmed-label = { $action }…
shell-session-row = Session
shell-session-row-confirms = Session · asks for confirmation
# Search words for session actions besides the English ones.
shell-session-lock-keywords = lock screen session
shell-session-log-out-keywords = log out sign out logout logoff exit quit session
shell-session-suspend-keywords = suspend sleep standby
shell-session-hibernate-keywords = hibernate sleep disk
shell-session-reboot-keywords = reboot restart
shell-session-power-off-keywords = power off shut down shutdown turn off halt

## Launcher: windows, files and controls

shell-window-workspace = Workspace { $workspace }
shell-window-workspace-on-output = Workspace { $workspace } · { $output }
shell-window-workspace-unavailable = Workspace unavailable
shell-window-switch = Switch to window
shell-window-launch-application = Launch application
shell-window-description = Window · { $application } · { $workspace }
shell-files-index-partial = File index is partial (scan limit reached)
shell-files-locations-unreadable = Some search locations could not be read
shell-files-top-matches = Showing the highest-ranked file matches.
# $parent is the folder that contains it.
shell-files-folder-in = Folder · { $parent }
shell-files-file-in = File · { $parent }
shell-files-copy-path = Copy path
shell-files-open-folder = Open containing folder
shell-files-open-with = Open with…
shell-files-quick-look = Quick Look
shell-control-description = Settings and controls
shell-control-wifi = Wi-Fi networks
shell-control-sound = Sound and volume
shell-control-notifications = Notifications and Do Not Disturb
shell-control-settings = Canopa Settings
shell-control-clear-history = Clear launcher history
shell-control-bluetooth = Bluetooth devices
shell-control-power = Power & brightness
# Search words besides the English ones.
shell-control-wifi-keywords = wifi wireless
shell-control-sound-keywords = sound audio
shell-control-notifications-keywords = notifications
shell-control-settings-keywords = settings
shell-control-clear-history-keywords = history
shell-control-bluetooth-keywords = bluetooth
shell-control-power-keywords = power brightness
shell-settings-page = Settings page
# $page is a Settings page title.
shell-settings-page-label = { $page } settings
shell-settings-not-installed = Canopa Settings is not installed
shell-settings-pages-timed-out = Canopa Settings did not list its pages in time
shell-settings-pages-failed = Canopa Settings could not list its pages
shell-settings-pages-too-large = Settings page list is too large
shell-settings-pages-invalid = Settings page list is invalid
shell-settings-no-such-page = Settings does not have that page

## Launcher: calculator, characters and extensions

shell-calculator-too-long = Calculations are limited to { $bytes } bytes
shell-calculator-target-unit = Choose a supported target unit
shell-calculator-amount = Enter a complete numerical amount
shell-calculator-source-unit = Choose a supported source unit
shell-calculator-dimensions = These units measure different quantities
shell-calculator-incomplete = Enter a complete arithmetic expression or unit conversion
shell-calculator-no-result-in-unit = Expression has no finite real result in the requested unit
shell-calculator-no-result = Expression has no finite real result
# $query is what the person typed.
shell-calculator-description = Calculator · { $query }
shell-calculator-copy-result = Copy result
shell-calculator-failed = Calculator: { $error } (requires qalc)
shell-characters-help = Search by name or keyword · Enter to copy
shell-provider-unavailable = Provider executable is unavailable
shell-provider-io-unavailable = Provider input or output is unavailable
shell-provider-io-failed = Provider input or output failed
shell-provider-output-too-large = Provider output exceeded 256 KiB
shell-provider-process-failed = Provider process failed
shell-provider-exit-failed = Provider exited unsuccessfully
shell-provider-timed-out = Provider timed out
shell-extension-web = Web
shell-extension-date-time = Date and time
# $keyword is the search's keyword.
shell-extension-web-search = Web · { $keyword }
shell-extension-duplicate-keyword = Duplicate extension keyword: { $keyword }
shell-extension-duplicate-search = Duplicate search keyword: { $keyword }
shell-extension-web-description = Paste an HTTP(S) URL, or use the keyword to search. Browser actions run only when selected.
# The example queries are typed in English.
shell-extension-date-time-description = Local dates, time zones and Unix timestamps. Try time in Tokyo, 90 days from today, or timestamp now.
shell-extension-configured-description = Configured executable extension. Its actions appear here after it returns results.
shell-extension-open-in-browser = Open in browser
shell-extension-copy-url = Copy URL
shell-extension-search-web = Search web: { $query }
shell-extension-web-result = Open in your default browser · requests start when selected
shell-extension-time-in = Time in { $zone }
shell-extension-unix-timestamp = Unix timestamp · seconds
shell-extension-timestamp-to-utc = Unix timestamp to UTC
shell-extension-date-to-timestamp = Date to Unix timestamp · seconds
shell-extension-calendar-date = Calendar date · local time zone

## Launcher: clipboard history and snippets

shell-clipboard-copy = Copy
shell-clipboard-remove = Remove from history
shell-clipboard-recent-copy = Recent copy
shell-clipboard-save-as-snippet = Save as snippet…
shell-clipboard-image = Image
# Searchable text of a copied image; $description is its format and size.
shell-clipboard-image-named = Image { $description }
# $format is PNG, JPEG, GIF or WebP; $width and $height are pixels.
shell-clipboard-image-description = { $format } · { $width } × { $height }
shell-clipboard-clear-history = Clear clipboard history
shell-clipboard-clear = Clear history
shell-clipboard-turn-off = Turn off clipboard history
shell-clipboard-turn-on = Turn on clipboard history
shell-clipboard-keep-recent = Keep recent copies for this session
shell-clipboard-rename-snippet = Rename snippet…
shell-clipboard-delete-snippet = Delete snippet
shell-clipboard-new-snippet = New snippet from clipboard
shell-clipboard-name-snippet = Name snippet…
shell-clipboard-loading = Loading clipboard preferences…
shell-clipboard-history-off = History is off · type snip for saved snippets
shell-clipboard-recording-text = Recording text for this session · Enter to copy · Tab for actions
shell-clipboard-recording-text-images = Recording text and images for this session · Enter to copy · Tab for actions
shell-clipboard-snippets-help = Saved snippets · Enter to copy · Tab to rename or delete
shell-clipboard-help = Enter to copy · Tab for actions
# $name is the snippet name being typed.
shell-clipboard-save-snippet-named = Save snippet “{ $name }”
shell-clipboard-save-snippet = Save snippet
shell-clipboard-entry-gone = That clipboard entry is no longer available
shell-clipboard-copy-text-first = Copy some text up to 16 KiB before creating a snippet
shell-clipboard-saving = A clipboard change is being saved
shell-clipboard-preferences-unavailable = Clipboard preferences are unavailable
shell-clipboard-save-stopped = Clipboard save stopped
shell-clipboard-reader-failed = Could not start the clipboard reader
shell-clipboard-wl-paste-failed = Could not start wl-paste for clipboard history
shell-clipboard-watcher-unreadable = Could not read the clipboard watcher
shell-clipboard-text-watcher-stopped = Text clipboard watcher stopped. Check Wayland data control support
shell-clipboard-image-watcher-stopped = Image clipboard watcher stopped. Check Wayland data control support
shell-clipboard-preferences-stopped = Clipboard preferences reader stopped
shell-clipboard-image-copy-failed = Could not copy the image to the clipboard
shell-history-unreadable = Launcher history could not be read
shell-history-invalid = Launcher history is invalid
shell-history-unsaved = Launcher history could not be saved
shell-history-unavailable = Launcher history worker is unavailable
shell-history-busy = Launcher history is busy

## Panel providers

# $provider is the provider's configured name.
shell-provider-event-timed-out = Provider “{ $provider }” did not answer in time
shell-provider-not-configured = Provider “{ $provider }” is not configured
shell-provider-not-available = Provider “{ $provider }” is not available
shell-provider-launch-failed = Could not start provider “{ $provider }”: { $error }
shell-provider-handshake-timed-out = Provider “{ $provider }” did not start in time
# A configuration notice when the configuration parses but its modules cannot be built.
shell-config-notice-construction = The Shell could not build the configured modules
# Accessible name of the launcher's scope tabs.
shell-launcher-search-in = Search in
