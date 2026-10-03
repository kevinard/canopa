# 0014: Apply the Canopa visual direction to the existing shell

Status: accepted, 2026-10-03.

The user selected the spine and shared visual theme as the first implementation
slice of `canopa-desktop-ui-ux.pdf`, and clarified that the existing bar is the
spine. This extends ADRs 0003 and 0013 through their existing appearance, Bar
Surface geometry, Module and Panel mechanisms.

Canopa's dark and light palettes replace the initial Catppuccin presets. The
existing `surface` role is the panel color; `spine`, `accent-surface` and `brand`
add the proposal's navigation, quiet selection and secondary identity colors.
Custom accents retain automatic contrasting filled-control text unless
`on-accent` is explicit. Typography defaults to installed Inter with system sans
fallback and 14/12/20 logical-pixel body, label and title sizes. Panels, cards and
controls share larger spacing and radii. Existing theme/local overrides apply.
Bar Panel module frames adopt the raised card color and shared card radius;
their existing internal padding stays aligned with the panel sizing estimates.

The bundled shell configuration makes the existing left Bar Surface a 64-pixel
frame with 12-pixel margins and a 28-pixel radius. Its stable exclusive zone is
88 pixels. Launcher and native Niri overview buttons occupy Start; existing
output-local workspace Modules occupy Center; notifications, system controls and
clock occupy End. Default single-icon targets are 44 pixels high; numeric
workspace labels and the text-only clock also have at least 44-pixel targets. Explicit user
bar layouts and the empty-schema 48-pixel Bar defaults are retained.

Embedded monochrome SVG icons are available to unrotated module icon
rendering as `canopa:mark`, `canopa:overview`, `canopa:controls`, `canopa:lock`,
`canopa:logout`, `canopa:reboot` and `canopa:poweroff`. They follow
configured size and color. No font download, asset lookup, module kind or
additional navigation model is introduced.

The launcher opens the existing installed-app Panel Menu. Controls open the
existing Bar Panel with network, sound, notifications, idle inhibitor, battery
and media modules, using
a configurable `panel.title` so it is not mislabeled Overview. Module-detail and
menu titles retain their content ownership. Explicit `open-bar-panel` and
`toggle-bar-panel` actions select the current bar's content while retaining the
originating module's anchor and focus return target. Existing scoped panel
actions keep their meaning. Panels remain overlays without
additional reservation. Foreground retains replacement, dismissal, focus and
animation ownership; Notify retains attention policy.

Bar Panel Modules may provide compact controls separately from full Module Detail
Content. Wi-Fi and Sound cards stay available when their status sources cannot
provide data, and activate the existing configured provider actions. Sound adds
volume step controls through the existing adjustment path. Notifications exposes
its secondary DND action as the quick toggle; the Idle Inhibitor exposes its
existing manual preference. Calendar stays on the clock. App resolves card input
against the current Panel window and layout. Provider navigation retains its
origin and anchor even when the activated Module has no Bar placement; scoped
detail actions select the activated Module. Panel Runtime owns card focus and
a one-level return to the originating Bar Panel; provider updates preserve that
return, while replacement from another origin and dismissal discard it. Pointer
Back and the existing Back key return; Escape closes. Providers continue owning
Wi-Fi connection and audio-device controls and process lifetimes.

Trailing Button Modules in a Bar Panel form a fixed two-column footer, retaining
the same visible module order and keyboard activation. The bundled footer has
Lock, Log out, Reboot and Power off. Lock hands `swaylock` to the desktop session;
the other controls open ordinary configured confirmation menus, with Cancel first
and returning to the Bar Panel. Confirmed logout uses Niri quit, and reboot/power
off use non-forced `systemctl` commands. No new Module Kind or session manager is
introduced. Custom button actions remain configurable.

The combined app/window/settings launcher, workspace inspector, rail overflow,
connector and urgency redesign, new controls, notification read/undo policy, and
motion specification remain later interaction slices. Current animations,
reduced-motion configuration and notification counts keep their meaning;
attention is not relabeled as unread history.

Validate contrast in both schemes, the real default configuration, native target
input, stable reservation during hover/panel interaction, keyboard dismissal and
fractional scale in the private desktop harness. Screenshots establish native
appearance, not the PDF's later acceptance journeys.
