# Canopa Settings. English is the source language.

## Window and sidebar

settings-window-title = Canopa Settings
# The small line above the sidebar heading.
settings-sidebar-brand = Canopa
settings-sidebar-heading = Settings
settings-sidebar-navigation = Settings pages
settings-sidebar-pages = Pages
settings-search-placeholder = Search settings
settings-search-none = No matching settings
# Read by screen readers while searching. $count is the number of pages found.
settings-search-found = { $count ->
    [one] { $count } matching page
   *[other] { $count } matching pages
}

## Sidebar groups

settings-group-devices = Devices
settings-group-input = Input
settings-group-appearance = Appearance
settings-group-system = System
settings-group-desktop = Desktop
# Debug builds only.
settings-group-developer = Developer

## Pages: the title in the sidebar and page header, the description under it,
## and search keywords. Keywords are a comma-separated lowercase list of words
## people may type in your language; the English keywords and titles always
## match too, so translate the meaning rather than every English word.

settings-page-displays-title = Displays
settings-page-displays-description = Manage connected monitors and save display layouts.
settings-page-displays-keywords = monitor, screen, resolution, refresh rate, scale, rotation, position, arrangement, profile, dock

settings-page-input-title = Keyboard & pointer
settings-page-input-description = Adjust keyboard, mouse, and touchpad behavior.
settings-page-input-keywords = keyboard, layout, xkb, repeat, mouse, touchpad, trackpad, pointer, tap to click, natural scrolling, left-handed

settings-page-network-title = Network
settings-page-network-description = Connect to Wi-Fi, wired networks and VPNs, and share a hotspot.
settings-page-network-keywords = wifi, wi-fi, wireless, internet, ethernet, wired, cable, ip address, dns, dhcp, vpn, wireguard, openvpn, ipsec, hotspot, tethering, eduroam, enterprise, 802.1x, certificate, metered, proxy, airplane

settings-page-bluetooth-title = Bluetooth
settings-page-bluetooth-description = Pair and connect headphones, keyboards, mice and other wireless devices.
settings-page-bluetooth-keywords = wireless, pair, pairing, headphones, headset, earbuds, speaker, keyboard, mouse, controller, gamepad, phone, battery, discoverable, visible, trust, forget, pin

settings-page-printers-title = Printers
settings-page-printers-description = Add printers, choose your default and manage print jobs.
settings-page-printers-keywords = print, printing, cups, queue, jobs, test page, ipp, airprint, driverless, scanner

settings-page-sound-title = Sound
settings-page-sound-description = Choose speakers and microphones, and set application volumes.
settings-page-sound-keywords = audio, volume, speakers, headphones, headset, microphone, input level, output, mute, balance, hdmi, bluetooth, a2dp, hfp, profile, applications, alert sound, notification sound

settings-page-personal-title = Personalization
settings-page-personal-description = Choose how your desktop and lock screen look and feel.
settings-page-personal-keywords = appearance, theme, dark, light, accent, color, wallpaper, background, lock screen, clock, text size, font, motion, animation

settings-page-accessibility-title = Accessibility
settings-page-accessibility-description = High contrast, larger text and cursor, reduced motion and the screen reader.
settings-page-accessibility-keywords = a11y, contrast, high contrast, text size, larger text, cursor, pointer size, motion, animation, screen reader, orca, vision

settings-page-power-title = Power
settings-page-power-description = Battery, power mode, brightness, the lid and idle timeouts.
settings-page-power-keywords = battery, health, charge, power mode, power profile, power saver, performance, brightness, backlight, lid, power button, idle, sleep, suspend, hibernate, lock, screen off, timeout, stasis

settings-page-date-time-title = Date & time
settings-page-date-time-description = Set the time zone, network time and the 12- or 24-hour clock.
settings-page-date-time-keywords = date, time, clock, time zone, timezone, ntp, network time, automatic, location, 12-hour, 24-hour, am, pm

settings-page-region-title = Region & language
settings-page-region-description = Choose your language and formats, and the system's language and keyboard.
settings-page-region-keywords = language, locale, formats, date format, number format, currency, measurement, metric, country, keyboard layout, login screen, translation

settings-page-users-title = Users
settings-page-users-description = Change your name, picture and password, and manage other accounts.
settings-page-users-keywords = accounts, user, password, picture, avatar, full name, administrator, admin, wheel, add user, remove user, family

settings-page-privacy-title = Privacy
settings-page-privacy-description = Review what applications may use, and what starts when you log in.
settings-page-privacy-keywords = permissions, location, camera, microphone, screenshot, screen sharing, screencast, remote desktop, background, autostart, startup, login items, portal, flatpak, revoke

settings-page-notifications-title = Notifications
settings-page-notifications-description = Do not disturb, its schedule, and each application's notifications.
settings-page-notifications-keywords = do not disturb, dnd, quiet, focus, schedule, banners, alerts, alarms, calls, fullscreen, screen sharing, lock screen, mute, sound, applications

settings-page-default-apps-title = Default applications
settings-page-default-apps-description = Choose which applications open web links, mail, music, files and more.
settings-page-default-apps-keywords = browser, web, mail, email, calendar, music, video, photos, images, text editor, file manager, terminal, file types, mime, open with, mimeapps

settings-page-launcher-title = Launcher
settings-page-launcher-description = Choose search providers and manage clipboard history.
settings-page-launcher-keywords = search, extensions, web search, clipboard, history, applications

settings-page-shortcuts-title = Shortcuts
settings-page-shortcuts-description = Find and edit keyboard shortcuts for your desktop.
settings-page-shortcuts-keywords = keyboard shortcuts, keybindings, hotkeys, bindings, keys

settings-page-widgets-title = Widgets
settings-page-widgets-description = Arrange widgets on your desktop and lock screen.
settings-page-widgets-keywords = bar, panel, modules, layout, lock screen, shell

settings-page-about-title = About
settings-page-about-description = This computer's name, hardware and software.
settings-page-about-keywords = system, information, device name, hostname, computer name, version, operating system, kernel, hardware, model, processor, cpu, memory, ram, graphics, gpu, disk, storage, firmware

# Debug builds only.
settings-page-live-example-title = Live page example
settings-page-live-example-description = The live service page pattern on a simulated service.


## Shared controls and the page footer

# Read by screen readers for a drop-down list: "3 of 8".
settings-choice-position = { $position } of { $count }
settings-applying = Applying…
settings-footer-label = Page actions
# Reloads only the page shown; other pages keep their edits.
settings-footer-reload = Reload this page and discard its edits
settings-footer-save = Save changes
settings-footer-test-displays = Test display changes
settings-saved = Settings saved.
# $page is a page title.
settings-activate-busy = Finish the current change, then open { $page }.
settings-session-required-title = Changes require a Canopa session
settings-session-required = Sign in to the Canopa desktop to save display and input settings.

## Displays

settings-displays-intro = Choose a mode, scale and position for each connected display.
settings-displays-refreshing = Refreshing…
settings-displays-reload = Reload connected displays
settings-displays-none = No connected displays are available.
settings-displays-changed-while-editing = Displays changed while you were editing. Reload connected displays to update the draft.
settings-displays-positions-hint = Positions use logical pixels. Keep at least one display enabled.
settings-display-enabled = Enabled
settings-display-mode = Resolution & refresh rate
# A display mode. $width and $height are pixels, $refresh the refresh rate in hertz.
settings-display-mode-option = { $width } × { $height } · { $refresh } Hz
settings-display-rotation = Rotation
# $scale is a formatted percentage, such as 125%.
settings-display-scale = Scale: { $scale }
settings-display-x = Horizontal position
settings-display-y = Vertical position
settings-rotation-normal = Normal
settings-rotation-90 = 90°
settings-rotation-180 = 180°
settings-rotation-270 = 270°
settings-rotation-flipped = Flipped
settings-rotation-flipped-90 = Flipped, 90°
settings-rotation-flipped-180 = Flipped, 180°
settings-rotation-flipped-270 = Flipped, 270°
# The 20-second test of new display settings.
settings-trial-title = Keep these display settings?
settings-trial-countdown = { $seconds ->
    [one] Keep these display settings? Reverting in { $seconds } second.
   *[other] Keep these display settings? Reverting in { $seconds } seconds.
}
settings-trial-keys = Ctrl+Enter keeps changes. Escape reverts.
settings-trial-revert = Revert
settings-trial-keep = Keep changes

## Keyboard & pointer

settings-input-keyboard = Keyboard
settings-input-mouse = Mouse
settings-input-touchpad = Touchpad
settings-input-layouts = Layouts, in switch order (for example us,fr)
settings-input-variants = Variants (optional, in the same order)
settings-input-options = XKB options (for example grp:alt_shift_toggle)
# $delay is in milliseconds.
settings-input-repeat-delay = Repeat delay: { $delay } ms
settings-input-repeat-rate = Repeat rate: { $rate } keys/second
# $speed is a number from -1 to 1.
settings-input-pointer-speed = Pointer speed: { $speed }
settings-input-touchpad-speed = Touchpad speed: { $speed }
settings-input-left-handed = Left-handed buttons
settings-input-mouse-natural-scroll = Natural scrolling for mouse
settings-input-tap = Tap to click
settings-input-touchpad-natural-scroll = Natural scrolling for touchpad
settings-input-disable-while-typing = Disable touchpad while typing
settings-power-key-unsaved-input = Save or reload your Keyboard & pointer edits before changing the power button.
settings-power-key-applied = Power button setting applied.

## Accessibility

settings-a11y-seeing = Seeing
settings-a11y-motion = Motion
settings-a11y-screen-reader = Screen reader
settings-a11y-high-contrast = High contrast
settings-a11y-high-contrast-hint = Black and white surfaces with full-strength text and borders. Applications that follow the desktop contrast setting change too.
# $scale is a formatted percentage.
settings-a11y-text-scale = Larger text · { $scale }
settings-a11y-cursor-size = Cursor size (applies at once)
settings-a11y-reduced-motion = Reduce motion in Canopa and in applications
settings-a11y-screen-reader-hint = Settings, the launcher, Files, the Shell bar and notifications work with the Orca screen reader. Start it with the command orca, or turn it on for every session in Orca’s own preferences.
# $size is in pixels.
settings-cursor-size-default = Default ({ $size } px)
settings-cursor-size-pixels = { $size } px
settings-cursor-size-needs-session = The cursor size is editable in a Canopa session.
settings-cursor-size-unsaved-input = Save or reload your Keyboard & pointer edits before changing the cursor size.
settings-cursor-size-applied = Cursor size applied. New windows use it at once.
settings-motion-use-niri = Use Niri animation settings
settings-motion-reduce = Reduce window and workspace motion
settings-motion-needs-session = Compositor motion is editable in a Canopa session with a managed Niri configuration.
settings-compositor-motion-unsaved-shortcuts = Save or discard your shortcut edits before changing compositor motion.
settings-compositor-motion-saved = Compositor motion settings saved.

## Personalization

settings-personal-appearance = Appearance
settings-personal-lock-screen = Lock screen
settings-personal-motion = Window and workspace motion
settings-personal-light = Light appearance
settings-personal-accent = Accent color (optional, #RRGGBB)
settings-personal-text-scale = Text size · { $scale }
settings-personal-reduced-motion = Reduce Shell panel motion
settings-personal-wallpaper = Choose wallpaper…
# $folder is a path.
settings-personal-wallpaper-hint = Preview and save wallpaper for each monitor. The default image folder is { $folder }.
settings-personal-clock-format = Clock format
settings-personal-date-format = Date format
# Under the clock and date format fields.
settings-personal-formats-hint = Leave a format empty to follow Region & language. Formats use strftime patterns, such as %H:%M.
settings-personal-font = Font family
settings-personal-avatar = Avatar image (optional absolute path)
settings-personal-dimming = Background dimming: { $dim }
settings-personal-edit-lock-widgets = Edit lock screen widgets…
settings-wallpaper-stopped = Wallpaper launcher stopped unexpectedly.
settings-alert-sounds-updated = Notification sounds updated.

## Power: idle manager

settings-idle-connecting = Connecting to the idle manager…
# $state and $profile are the idle manager's own names. $paused is yes or no.
settings-idle-status = State: { $state }    Profile: { $profile }    Paused manually: { $paused ->
    [yes] yes
   *[no] no
}
settings-idle-unavailable = Idle manager unavailable: { $error }
settings-idle-policy-updated = Idle policy updated.
settings-idle-profile-selected = Idle profile selected.
settings-saved-idle-not-running = Settings saved. No idle daemon is running; Stasis uses these timeouts when it starts.
settings-lid-saved-idle-failed = The lid choice was saved, but the idle timeouts were not: { $error }

## Launcher and shortcuts notices

settings-launcher-saved = Launcher preferences saved.
settings-clipboard-saved = Clipboard settings saved.
settings-shortcuts-saved = Keyboard shortcuts saved.

## Live pages: pages that follow a system service and apply each change at once.
## $service is a service name, such as NetworkManager or BlueZ.

settings-live-connecting = Connecting to { $service }…
settings-live-stopped = { $service } stopped.
settings-live-unavailable-title = Service not available
# $reason is a sentence explaining why, possibly empty.
settings-live-unavailable = { $service } is not installed or not running. { $reason }
settings-live-try-again = Try again
settings-live-retry = Retry
settings-live-dismiss = Dismiss
# Service errors shown on a control.
settings-error-not-authorized = The system did not allow this change.
settings-error-timed-out = The system did not answer in time.
settings-error-bus = The system bus is not reachable.

## Service errors. "Invalid" messages repeat what a system service refused.

settings-error-setting-changed = The setting changed. Try again.
# $what is the English name of a value, used only for values Canopa does not know.
settings-error-invalid-value = That is not a valid { $what }.
settings-invalid-device-name = That is not a valid device name.
settings-invalid-host-name = That is not a valid host name.
settings-invalid-full-name = That is not a valid full name.
settings-invalid-user-name = That is not a valid user name.
settings-invalid-picture = That is not a valid picture.
settings-invalid-language = That is not a valid language.
settings-invalid-formats = Those are not valid formats.
settings-invalid-keyboard-layout = That is not a valid keyboard layout.
settings-invalid-printer-name = That is not a valid printer name.
settings-invalid-printer-address = That is not a valid printer address.
settings-invalid-time-zone = That is not a valid time zone.
settings-invalid-time-before-1970 = The time cannot be before 1970.
settings-invalid-request = The request was not valid.
settings-invalid-dns = A DNS server is not a valid address.
settings-invalid-wifi-password = A Wi-Fi password has 8 to 63 characters.
settings-invalid-hotspot-password = A hotspot password has 8 to 63 characters.
settings-invalid-network-name = A network name has 1 to 32 bytes.
settings-invalid-preshared-key = A preshared key is not a WireGuard key.
settings-invalid-tunnel-peer = A tunnel needs a peer.
settings-invalid-address = An address is not valid.
settings-invalid-brightness = Brightness must be between 1% and 100%.
settings-invalid-certificate-path = Certificate paths must be UTF-8.
settings-invalid-eap-method = Choose an authentication method.
settings-invalid-password-missing = Enter the password.
settings-invalid-identity-missing = Enter your identity (user name).
# ListenPort, MTU and PersistentKeepalive are WireGuard setting names.
settings-invalid-listen-port = ListenPort is not a port.
settings-invalid-mtu = MTU is not a number.
settings-invalid-manual-address = Manual addressing needs an address.
settings-invalid-connection-name = Name the connection.
settings-invalid-keepalive = PersistentKeepalive is not a number.
settings-invalid-peer-setting = The file has an unknown peer setting.
settings-invalid-section = The file has an unknown section.
settings-invalid-wireguard-file = The file is not a WireGuard configuration.
settings-invalid-gateway = The gateway is not a valid address.
settings-invalid-prefix = The prefix must be a number.
settings-invalid-private-key = The private key is not a WireGuard key.
settings-invalid-address-method = Unknown address method.
settings-invalid-proxy = Write the proxy as host:port.
settings-invalid-wayland-login = A Wayland login is required.
settings-invalid-local-login = An active local login is required.
settings-invalid-keyboard-brightness = Keyboard brightness is above the device’s range.
settings-invalid-missing-file = Choose a certificate or key file that exists.
settings-invalid-tls-files = TLS needs your certificate and private key.
settings-invalid-peer-key = A peer’s public key is not a WireGuard key.
settings-invalid-address-prefix = Write addresses with a prefix, as in 192.168.1.10/24.
settings-invalid-proxy-url = The configuration address must start with http://, https:// or file://.

## Region & language

settings-region-yours = Your language and formats
settings-region-system = System
settings-region-language = Language
settings-region-formats = Formats (dates, numbers, money)
# The name of the C locale, which has no language or country.
settings-region-unspecified-locale = Unspecified (C)
settings-region-only-c = Only the C locale is installed. An administrator adds languages by enabling them in /etc/locale.gen and running locale-gen.
settings-region-preview-date = Date
settings-region-preview-time = Time
settings-region-preview-number = Number
settings-region-preview-money = Money
settings-region-preview-measurement = Measurement
# $long is a full date, $short the same date in numbers.
settings-region-preview-dates = { $long } · { $short }
settings-region-metric = Metric
settings-region-us-customary = US customary
# $file is a path.
settings-region-applies = Settings changes at once. Files, Notify and the Shell follow when they restart, and other applications at your next login. Saved in { $file }.
settings-region-no-localed = systemd-localed is not running, so the system language and keyboard cannot be shown or changed here.
# $layout is an XKB layout name.
settings-region-keyboard-unset = Not set ({ $layout })
settings-region-system-language = Login screen and new users
settings-region-system-keyboard = System keyboard
settings-region-your-layouts = Your layouts
settings-region-use-my-layouts = Use my layouts for the login screen and console
settings-region-change-layouts = Change my layouts…
settings-region-system-permission = System changes ask for an administrator’s permission.
# The name of a language Canopa is translated into, written in the current language. $code is its code.
settings-language-name = { $code ->
    [fr] French
   *[en] English
}
# After the person chooses a language. $language is from settings-language-name.
settings-region-language-applied = Settings now uses { $language }. Restart Files, Notify and the Shell, or log out, to see the change everywhere; other applications follow at your next login.
# $locale is a locale name such as "German (Germany)".
settings-region-language-untranslated = Canopa is not translated into this language ({ $locale }) yet, so Settings stays in English. Other applications follow your choice at your next login.
# $formats is a locale name such as "French (France)".
settings-region-formats-applied = Settings now shows dates and numbers in the { $formats } formats. Restart Files, Notify and the Shell, or log out, to see the change everywhere; other applications follow at your next login.
settings-region-error-no-localed = systemd-localed is not available.
settings-region-error-no-config = There is no configuration folder.
settings-region-error-save = locale.conf could not be saved: { $error }
settings-region-error-not-installed = That language is not installed.

## Date & time

settings-date-time-zone = Time zone
# $zone is a time zone name such as "Europe/Paris".
settings-date-time-zone-current = Time zone: { $zone }
settings-date-time-zone-unset = Not set (UTC)
# The current date and time. $date is a full date, $time a time with seconds.
settings-date-time-now = { $date } · { $time }
settings-date-time-locate = Use my location
settings-date-time-locate-hint = Sets the time zone of this computer’s location once, through GeoClue.
settings-date-time-zone-search = Find a time zone (city or region)
settings-date-time-zone-none = No time zone matches.
settings-date-time-zone-in-use = In use
settings-date-time-zone-use = Use
settings-date-time-clock = Date and time
settings-date-time-ntp = Set the time automatically from the network
settings-date-time-ntp-off = Off: the clock keeps the time set below.
settings-date-time-ntp-synchronized = The clock is synchronized.
settings-date-time-ntp-waiting = Waiting for a time server…
settings-date-time-ntp-missing = No network time service (systemd-timesyncd, chrony or ntpd) is installed.
# The fields take these exact forms whatever the language.
settings-date-time-date-field = Date (YYYY-MM-DD)
settings-date-time-time-field = Time (HH:MM)
settings-date-time-set = Set date and time
settings-date-time-bad-entry = Enter the date as YYYY-MM-DD and the time as HH:MM.
settings-date-time-local-rtc = The hardware clock keeps local time, as Windows does; this can confuse daylight saving changes.
settings-date-time-not-running = systemd-timedated did not answer.
settings-date-time-location-denied = Location access is not allowed for Canopa Settings.
settings-date-time-location-timeout = The location could not be found in time.
settings-date-time-location-unavailable = The location is not available.
settings-date-time-no-tzdata = The time zone table (tzdata) is not installed.
settings-date-time-no-zone-here = No time zone matches this location.

## Clock format (Date & time page)

settings-clock-format = Clock format
settings-clock-reading = Reading config.kdl…
settings-clock-unreadable = The clock formats are set in config.kdl, which could not be read: { $error }
# Clocks follow the language's formats chosen on Region & language.
settings-clock-automatic = Automatic
# $example is 2:30 PM written as the choice writes it.
settings-clock-24 = 24-hour ({ $example })
settings-clock-12 = 12-hour ({ $example })
settings-clock-automatic-24 = Automatic follows Region & language, which uses a 24-hour clock ({ $example }).
settings-clock-automatic-12 = Automatic follows Region & language, which uses a 12-hour clock ({ $example }).
settings-clock-applies = For the bar clock and the lock screen. Applies at once.
# $place is settings-clock-place-shell or settings-clock-place-lock; $format a pattern such as %H.%M.
settings-clock-custom = { $place } uses its own format, { $format }, which is kept.
# $name is the clock module's name in config.kdl.
settings-clock-place-shell = Shell clock “{ $name }”
settings-clock-place-lock = Lock screen
settings-clock-updated = Clock format updated.

## Network

settings-cancel = Cancel
settings-network-not-running = NetworkManager is not running.
settings-network-error-changed = The connection changed. Try again.
settings-network-error-timed-out = NetworkManager did not answer in time.
settings-network-needs-password = This network needs a password.
settings-network-question-gone = NetworkManager no longer needs this.
settings-network-import-choose-kind = Choose the kind of VPN this file is for.
settings-network-import-unknown-kind = Unknown VPN kind.
settings-network-import-timed-out = Importing took too long.
settings-network-import-needs-nmcli = nmcli is needed to import this VPN file.
# $error is nmcli's own message.
settings-network-import-failed = The VPN file could not be imported: { $error }
settings-network-no-agent = Settings could not offer NetworkManager its secret agent; connections that ask for a password each time cannot connect from here.
# Wi-Fi security, as chosen when joining a network.
settings-network-security = Security
settings-network-security-open = Open
settings-network-security-personal = WPA/WPA2 Personal
settings-network-security-sae = WPA3 Personal
settings-network-security-enterprise = WPA-Enterprise
settings-network-eap = Authentication
settings-network-eap-tls = TLS (certificate)
settings-network-phase2 = Inner authentication
settings-network-identity = Identity (user name)
settings-network-anonymous = Anonymous identity (optional)
settings-network-ca = CA certificate (empty: system certificates)
settings-network-domain = Server domain (recommended)
settings-network-certificate = Your certificate
settings-network-private-key = Your private key
settings-network-key-password = Private key password
settings-network-password = Password
settings-network-wifi-password = Wi-Fi password
settings-network-remember = Remember the password (NetworkManager stores it)
settings-network-choose = Choose…
# Titles of the file chooser.
settings-network-choose-ca = Choose the CA certificate
settings-network-choose-certificate = Choose your certificate
settings-network-choose-key = Choose your private key
settings-network-choose-vpn = Choose a VPN file
settings-network-name = Network name
# $network is a Wi-Fi network name.
settings-network-join-named = Join { $network }
settings-network-join = Join
settings-network-join-hidden = Join a hidden network…
settings-network-connect = Connect
# Connect after asking for a password.
settings-network-connect-ask = Connect…
settings-network-disconnect = Disconnect
settings-network-connecting = Connecting…
settings-network-disconnected = Disconnected
settings-network-question-title = Sign-in needed
# $connection is a connection name.
settings-network-question = NetworkManager needs this to connect { $connection }.
settings-network-question-retry = The previous answer for { $connection } was not accepted. Try again.
settings-network-question-hint = Settings passes this to NetworkManager and does not keep it.
settings-network-no-wifi = No Wi-Fi device.
settings-network-wifi-hardware-off = A hardware switch or airplane-mode key turned Wi-Fi off.
settings-network-none-found = No networks found yet.
# A network's security, shown in its description.
settings-network-ap-open = open
settings-network-ap-secured = secured
settings-network-ap-enterprise = enterprise
# A nearby network. $security is settings-network-ap-*, $strength a formatted percentage.
settings-network-ap-description = { $state ->
    [connected] Connected · { $security } · { $strength }
    [saved] Saved · { $security } · { $strength }
   *[other] { $security } · { $strength }
}
settings-network-kind-hotspot = hotspot
settings-network-kind-wired = wired
# A saved connection in a list. $kind is Wi-Fi, wired, WireGuard and so on.
settings-network-connection-option = { $name } ({ $kind })
settings-network-wired = Wired
# $name is the connection, $addresses its IP addresses.
settings-network-wired-connected = Connected · { $name } · { $addresses }
settings-network-wired-unplugged = Cable unplugged
settings-network-no-wired = No wired network device.
settings-network-wired-hint = Change a wired connection’s addresses and DNS under Connection settings.
# $connected is yes or no.
settings-network-vpn-status = { $connected ->
    [yes] { $kind } · Connected
   *[no] { $kind } · Not connected
}
settings-network-no-vpn = No VPN connections.
settings-network-wg-open = Add a WireGuard tunnel…
settings-network-wg-name = Name
settings-network-wg-addresses = Addresses (for example 10.0.0.2/24)
settings-network-wg-dns = DNS servers (optional)
settings-network-wg-peer = Peer public key
settings-network-wg-endpoint = Peer endpoint (host:port)
settings-network-wg-allowed = Allowed IPs (for example 0.0.0.0/0)
settings-network-add = Add
settings-network-import-title = Import a VPN file
settings-network-import-file = File (.conf, .ovpn, …)
settings-network-import-detect = Detect WireGuard
settings-network-import = Import
settings-network-import-no-plugins = OpenVPN and IPsec files need their NetworkManager plugin, such as networkmanager-openvpn.
settings-network-import-plugins = Plugin VPN files are read by the installed NetworkManager plugin.
settings-network-hotspot = Hotspot
settings-network-hotspot-no-wifi = A hotspot needs a Wi-Fi device.
# $name is the hotspot's network name.
settings-network-hotspot-sharing = Sharing as { $name }
settings-network-hotspot-stop = Stop the hotspot
settings-network-hotspot-start = Start the hotspot
settings-network-hotspot-password = Password (8 to 63 characters)
settings-network-hotspot-hint = Wi-Fi disconnects from its network while the hotspot runs.
settings-network-connections = Connection settings
settings-network-no-connections = No saved connections.
settings-network-choose-connection = Choose a saved connection.
settings-network-autoconnect = Connect automatically
settings-network-metered = Metered connection (limits background downloads)
settings-network-metered-auto = Automatic
settings-network-metered-yes = Metered
settings-network-metered-no = Not metered
settings-network-ip-dhcp = Automatic (DHCP)
settings-network-ip-auto = Automatic
settings-network-ip-manual = Manual
settings-network-ip-disabled = Disabled
settings-network-ip-addresses = Addresses with prefix (for example 192.168.1.10/24)
settings-network-ip-gateway = Gateway
settings-network-ip-dns = DNS servers (empty: from the network)
settings-network-apply = Apply
settings-network-proxy = Proxy
settings-network-proxy-none = No proxy
settings-network-proxy-auto = Automatic (configuration URL)
settings-network-proxy-manual = Manual
settings-network-proxy-server = Proxy server (host:port)
settings-network-proxy-url = Configuration URL (PAC)
settings-network-proxy-hint = Applications that ask the system proxy (PacRunner, libproxy) use this.
settings-network-forget = Forget this connection

## Service names, in sentences such as "Connecting to the audio server…"

settings-service-power = The power services
settings-service-audio = The audio server
settings-service-permission-store = The portal permission store
settings-service-applications = The application list
settings-error-device-changed = The device changed. Try again.
settings-error-service-timed-out = The service did not answer in time.

## Durations of idle stages

settings-duration-seconds = { $seconds ->
    [one] { $seconds } second
   *[other] { $seconds } seconds
}
settings-duration-minutes = { $minutes ->
    [one] { $minutes } minute
   *[other] { $minutes } minutes
}
settings-duration-hours = { $hours ->
    [one] { $hours } hour
   *[other] { $hours } hours
}
settings-duration-hours-minutes = { $hours ->
    [one] { $hours } hour
   *[other] { $hours } hours
} { $minutes ->
    [one] { $minutes } minute
   *[other] { $minutes } minutes
}
# $duration is one of the durations above.
settings-duration-and-seconds = { $duration } { $seconds ->
    [one] { $seconds } second
   *[other] { $seconds } seconds
}

## Power

settings-battery = Battery
settings-battery-no-upower = Battery information is not available because UPower is not running.
settings-battery-none = This computer has no battery.
settings-battery-charging = Charging
settings-battery-discharging = On battery
settings-battery-empty = Empty
settings-battery-full = Fully charged
settings-battery-pending-charge = Plugged in, not charging
settings-battery-pending-discharge = Waiting to discharge
settings-battery-unknown = Charge state unknown
# $level is a formatted percentage, $state a charge state above, $time a duration.
settings-battery-status = { $level } · { $state }
settings-battery-status-left = { $level } · { $state } · about { $time } left
settings-battery-status-to-full = { $level } · { $state } · full in about { $time }
# $full and $design are watt-hours.
settings-battery-health-energy = Health: { $capacity } of its original capacity ({ $full } of { $design } Wh)
settings-battery-health = Health: { $capacity } of its original capacity
settings-battery-health-unknown = The battery does not report its health.
settings-battery-cycles = Charge cycles: { $cycles }
settings-power-mode = Power mode
settings-power-modes-unavailable = Power modes are not available because power-profiles-daemon is not running.
settings-power-mode-current = Current mode: { $mode }
settings-power-mode-saver = Power saver
settings-power-mode-balanced = Balanced
settings-power-mode-performance = Performance
settings-power-error-no-modes = Power modes are not available.
settings-power-error-no-backlight = There is no built-in display backlight.
settings-power-error-brightness = The brightness could not be changed in this session.
settings-power-error-mode = The power mode could not be changed.
settings-brightness = Brightness
settings-brightness-none = This computer has no built-in display backlight.
settings-brightness-built-in = Built-in display: { $value }
settings-power-lid-and-button = Lid and power button
settings-power-lid-closed = When the lid is closed
settings-power-lid-hint = Applies after Save, in the Canopa session. While an external display is connected, closing the lid does nothing.
settings-power-button-pressed = When the power button is pressed
settings-power-button-hint = Applies at once. The system setting is usually Power off.
settings-power-button-needs-session = The power button can be changed in a Canopa session.
settings-power-system-setting = Use the system setting
settings-power-suspend = Suspend
settings-power-hibernate = Hibernate
settings-power-lock = Lock the screen
settings-power-nothing = Do nothing
settings-power-off = Power off
settings-idle = Idle
settings-idle-lock-after = Lock the screen after
settings-idle-displays-after = Then turn off the displays after
settings-idle-suspend-after = Then suspend after
# $status is settings-idle-status.
settings-idle-running = Idle daemon: running. { $status }
settings-idle-not-running = Idle daemon: not running.
settings-idle-optional = Stasis is optional. You can still save timeouts; they take effect when Stasis starts.
settings-idle-pause = Pause idle actions
settings-idle-resume = Resume
settings-idle-profile = Stasis profile
settings-idle-use-profile = Use profile
settings-idle-stages-hint = Each stage waits after the previous one: lock, displays off, then suspend.
settings-idle-ac = On AC power
settings-idle-battery = On battery
settings-idle-custom-hint = Custom commands, profiles and inhibition rules stay in your existing idle policy.

## Users

settings-users-not-running = AccountsService is not running.
settings-users-timed-out = AccountsService did not answer in time.
settings-users-changed = The account changed. Try again.
settings-users-gone = The account no longer exists.
settings-users-last-administrator = Another account must be an administrator first.
settings-users-own-password = Change your own password with your current one.
settings-users-remove-self = You cannot remove the account you are using.
settings-users-yours = Your account
settings-users-others = Other users
settings-users-unknown-me = AccountsService does not know your account.
settings-users-no-others = No other accounts.
settings-users-others-hint = Changing other accounts asks for an administrator’s password.
# $name is the user name; $administrator is yes or no.
settings-users-me = { $administrator ->
    [yes] { $name } · Administrator
   *[no] { $name } · Standard account
}
settings-users-full-name = Full name
settings-users-user-name = User name
settings-users-save-name = Save name
settings-users-picture-none = Picture: none, initials are shown
# $file is a path.
settings-users-picture = Picture: { $file }
settings-users-choose-picture = Choose a picture
settings-users-choose-picture-button = Choose picture…
settings-users-remove-picture = Remove
settings-users-picture-hint = A PNG or JPEG picture up to 2 MB and 2048 pixels a side, as the login screen shows.
settings-users-picture-unreadable = The picture cannot be read.
settings-users-picture-not-file = Choose an image file.
# $size is a formatted size.
settings-users-picture-too-large = Choose a picture smaller than { $size }.
settings-users-picture-format = Choose a PNG or JPEG picture.
settings-users-picture-too-big = Choose a picture at most { $pixels } pixels wide and high.
settings-users-administrator = Administrator
settings-users-administrator-toggle = Administrator: can install software and change system settings
settings-users-password-current = Current password
settings-users-password-new = New password
settings-users-password-confirm = New password again
settings-users-password-change = Change password
settings-users-password-changing = Changing…
settings-users-password-changed = Your password was changed.
settings-users-password-hint = The system checks your current password and its password rules, as passwd does in a terminal.
settings-users-password-mismatch = The new passwords do not match.
# $reason is passwd's English explanation.
settings-users-password-weak = The new password is too weak: { $reason }.
settings-users-password-wrong = The current password is not correct.
settings-users-password-too-soon = The password was changed recently; the system asks to wait before changing it again.
settings-users-password-failed = The password could not be changed.
settings-users-passwd-missing = passwd is not installed.
settings-users-passwd-failed = passwd stopped unexpectedly.
settings-users-passwd-timed-out = Changing the password took too long.
settings-users-remove = Remove…
settings-users-password-at-login = Chooses a new password at the next login
settings-users-password-none = No password
settings-users-locked = Account locked
settings-users-password-set = Has a password
settings-users-ask-new-password = Ask for a new password at next login
# $name is the person's name, $home their home folder.
settings-users-remove-question = Remove { $name }? Their files in { $home } can be kept or deleted.
settings-users-remove-question-no-home = Remove { $name }? Their files in their home folder can be kept or deleted.
settings-users-remove-keep = Remove, keep files
settings-users-remove-delete = Remove and delete files
settings-users-add-open = Add a user…
settings-users-name-rules = Lowercase letters, digits, - and _. The home folder is /home/<user name>.
settings-users-name-invalid = Start with a lowercase letter; use lowercase letters, digits, - and _ (at most 32).
settings-users-add-hint = They choose their password the first time they log in.
settings-users-add = Add

## Notifications

settings-notifications-not-running = Canopa Notify is not running.
settings-notifications-too-old = The running Canopa Notify has no preferences control yet; restart it after updating Canopa.
# $reason is Canopa Notify's explanation.
settings-notifications-refused = Canopa Notify refused the change: { $reason }
settings-notifications-bad-reply = Canopa Notify sent a reply Settings does not understand.
settings-dnd = Do not disturb
# $time is a time of day, possibly with settings-notifications-tomorrow.
settings-dnd-off-until = Off. Turns on at { $time }.
settings-dnd-off = Off. Notifications show banners and play sounds.
settings-dnd-on-manual = On until you turn it off.
settings-dnd-on-schedule = On by the schedule until { $time }.
settings-dnd-on-until = On until { $time }.
settings-dnd-on = On.
settings-notifications-tomorrow = tomorrow { $time }
settings-notifications-later = later
settings-dnd-for-hours = { $hours ->
    [one] For { $hours } hour
   *[other] For { $hours } hours
}
settings-dnd-until-tomorrow = Until tomorrow morning
settings-dnd-schedule = Turn on automatically every day
# "From 22:00 to 07:00", with time choices between.
settings-dnd-from = From
settings-dnd-to = to
settings-dnd-ends-next-morning = The quiet time ends the next morning.
settings-dnd-ends-same-day = The quiet time ends the same day.
settings-dnd-no-days = Choose at least one day for the schedule.
settings-dnd-break-through = While Do not disturb is on, still show
settings-dnd-alarms = Alarms
settings-dnd-calls = Incoming calls
settings-dnd-from-config = These come from config.kdl. Changing them here overrides that file.
settings-dnd-use-config = Use the settings from config.kdl
settings-suppression = Hide banners automatically
settings-suppression-fullscreen = While an application is fullscreen
settings-suppression-sharing = While sharing or recording the screen (content is hidden)
settings-suppression-now-sharing = Banners are hidden now because the screen is being shared.
settings-suppression-now-fullscreen = Banners are hidden now because an application is fullscreen.
settings-suppression-none = Notifications still reach the Notification Center.
settings-notifications-apps = Applications
settings-notifications-apps-hint = Lock-screen visibility applies to urgent notifications, the only ones the lock screen shows.
settings-notifications-no-rules = No application has its own rules yet. Find one below to set them.
settings-notifications-search = Find an application
settings-notifications-reading-apps = Reading installed applications…
settings-notifications-no-match = No other installed application matches.
settings-notifications-reset = Reset
settings-notifications-delivery = Notifications
settings-notifications-delivery-default = Default
settings-notifications-delivery-banners = Show banners
settings-notifications-delivery-center = Notification Center only
settings-notifications-delivery-mute = Mute
settings-notifications-sounds = Play sounds
settings-notifications-lock = On the lock screen
settings-notifications-lock-hide = Hide
settings-notifications-lock-name = Name only
settings-notifications-lock-content = Show content
settings-notifications-configured = config.kdl also sets rules for this application.
