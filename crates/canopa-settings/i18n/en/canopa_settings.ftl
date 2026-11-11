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
