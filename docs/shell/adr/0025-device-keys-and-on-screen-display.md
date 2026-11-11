# Device keys and the on-screen display

Date: 2026-11-11
Status: accepted

Volume keys ran `wpctl` with nothing shown, could push the level past 100 %, and
brightness, keyboard-backlight and media keys did nothing. The Shell now owns
these keys as **device keys** and shows their result in an **on-screen display**
(OSD).

## Commands

`canopa-shell volume up|down|mute`, `microphone mute` (alias `mic`),
`brightness up|down`, `keyboard-backlight up|down|toggle` and
`media play-pause|play|pause|stop|next|previous` send a typed `device-key`
request over the existing control socket, like `reload` and `--open-menu`. The
Shell answers once the key has run. Volume and brightness accept an optional step
(`canopa-shell volume up 10`) that overrides the configured one. No D-Bus method
is added: the socket is the Shell's command surface, and Niri spawns the
commands.

When no Shell owns the socket, the command runs the key itself with the
configured steps and no display, so the keys keep working while the Shell
restarts. A missing service is an ordinary result: the command exits 1 and
prints nothing, so held keys never fill the compositor's log.

The actions live in the Shell library (`device_keys`), which depends only on
`canopa-services` ([ADR 0137](../../adr/0137-shared-system-service-clients.md)):

- **Volume** changes the default output or input through the Pulse-protocol
  client, not `wpctl`. Raise adds the step to the loudest channel, keeps channel
  proportions, stops at 100 % and unmutes; lower keeps mute; an output another
  program amplified past 100 % is not raised further. Changes are serialized, so
  key repeat loses no step.
- **Brightness** uses logind `SetBrightness` on the session, without root. Steps
  are perceptual (a gamma-2 curve), so each press looks like the same change.
  Each press moves at least one hardware level, and a press down never goes below
  1 % of the range, so the panel never goes dark by accident. Requests revalidate
  the device and run one at a time.
- **Keyboard backlight** steps every `*::kbd_backlight` LED by one level (a tenth
  of the range on fine ones) through logind; toggle turns it off and back on at
  the level it turned off, or the maximum.
- **Media** controls the Shell's selected player through
  `org.canopa.Shell.Media` ([ADR 0018](0018-mpris-media-controls.md)), so keys,
  bar and panel agree on the player, including a pinned one. Without a media
  source (no `media` module) the key picks the player automatic selection would:
  playing, then paused. Play-pause pauses a playing player and plays any other;
  MPRIS gained `Stop`. Controls revalidate the player as before. Media keys show
  nothing.

## Display

The OSD is a layer-shell surface on the overlay layer, on Niri's focused output
(or the compositor's choice before Niri reports one), anchored above the bottom
edge and respecting bar reservations. It has no keyboard interactivity and an
empty input region, so it never takes focus and clicks pass through. It is
separate from the Foreground ([ADR 0008](0008-model-interactive-overlays-as-foreground.md))
because it is passive and must not dismiss or be dismissed by panels.

It shows an icon, the output or input device name (or the channel), a level bar
and a value: a percentage, `Muted`, `Off` or `Unavailable`. One indicator is
visible at a time. A press replaces it and restarts its timeout (1.5 s by
default), so repeated presses never stack. It uses the shared appearance's
notification style and palette, and fades out over 180 ms unless reduced motion
is set. The OSD has its own AccessKit adapter with one `Status` node in a polite
live region; AccessKit emits an AT-SPI announcement when its text changes, for
example "Volume 45%, Speakers".

A missing service shows `Unavailable` briefly after a press. Nothing else is
reported.

## Outside changes

The display also follows changes made elsewhere:

- the default output and input, from an event-driven Pulse subscription that
  parks between server events (`audio::watch_defaults`);
- the display backlight, from the kernel's notification on `actual_brightness`,
  which it sends for firmware hotkeys and for writes through sysfs or logind by
  other programs;
- keyboard backlights, from `brightness_hw_changed`, which drivers that support
  it notify for firmware changes. Writes by other programs to an LED are not
  notified and are not shown.

None of these polls; the Shell has no new idle wakeups. The first reading after
start, and after a service returns, is a baseline and shows nothing, so nothing
appears at login. Readings within 750 ms of a press on the same channel are its
echo: they correct the visible value without restarting it. Other changes show
once they have been quiet for 120 ms, or after 400 ms if they keep changing, so
a slider drag in another application shows one indicator that follows it. A new
default output (for example headphones) shows with its name. Microphone levels
alone never show: voice applications adjust them all the time; mute does show.

## Configuration and bindings

An optional `osd` block in the Shell namespace sets `enabled` (the keys work
either way), `volume-step` (1–100, default 5), `brightness-step` (1–50,
default 5) and `timeout-ms` (500–10000, default 1500). It reloads live.

The shipped Niri defaults bind `XF86AudioRaiseVolume`, `XF86AudioLowerVolume`,
`XF86AudioMute`, `XF86AudioMicMute`, `XF86MonBrightnessUp/Down`,
`XF86KbdBrightnessUp/Down`, `XF86KbdLightOnOff` (the keyboard-backlight toggle;
xkbcommon has no `XF86KbdBrightnessToggle`) and `XF86AudioPlay`, `Pause`,
`Stop`, `Next`, `Prev`. Volume, brightness and media keys carry
`allow-when-locked=true`; mute, toggle and media keys carry `repeat=false`.
Because Settings stores shortcut changes over the shipped defaults
([ADR 0135](../../adr/0135-shortcut-changes-over-shipped-defaults.md)), every
user receives the new bindings at their next login. Saved changes that still
name one of the four retired `wpctl` actions follow it to its Shell command, so
a moved key stays moved and a turned-off key stays off; unchanged copies of the
old defaults become inherited.

## Not covered

External monitors' brightness (DDC/CI) and per-application volume remain out
of scope. The Power & brightness panel slider still shows linear percent.
While the session is locked the keys work but the lock screen covers the
display.
