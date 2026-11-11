# 0144 A shared AccessKit layer for Canopa's Iced windows

Status: accepted. Builds on [ADR 0125](0125-shared-authentication-input-and-accessibility.md),
[ADR 0003](0003-shared-appearance.md) and [ADR 0008](0008-settings-portal.md);
Shell provider panels keep their bridge from the
[Shell accessibility record](../shell/ACCESSIBILITY.md).

Before this, three AT-SPI integrations existed, each hand-building an AccessKit
tree from its own model: the Shell's provider panels, Notify's banners and the
shared lock/greeter input. Settings, the launcher, Files, the bar and portal
dialogs had none, and Iced 0.14 (including our fork) has no accessibility of its
own. Writing a model-side tree for every Settings page, launcher view and Files
view would duplicate each view and drift from it.

## Options considered

- **One hand-written tree per application**, as the Shell and Notify do.
  Exact, but every view is described twice; Settings alone has twenty pages.
- **Accessibility inside Iced** (a fork topic that gives each widget a role).
  The upstream design is still open; a fork-wide change to every widget is a
  large divergence to carry and would not know our domain names or actions.
- **Declarative semantics beside the widgets, read from the laid-out tree.**
  A view wraps a control with a `Node` saying what it is; one Iced widget
  operation reads those nodes together with the text drawn inside them, the
  keyboard-focused widget and every node's layout bounds. Chosen.

## Decision

`canopa-ui` gains an `accessibility` feature (`accesskit`, `accesskit_unix`)
and an `accessibility-testing` feature with private-bus helpers. Core profiles
do not enable it (`scripts/check-core-dependencies.sh` still passes).

- **Semantics.** `semantic(Node, element)` is a transparent wrapper: it shares
  the wrapped widget's tree state, so semantics never reset focus, scrolling or
  drafts. A `Node` has a role, a stable key, name, value, description, numeric
  range, toggled/selected/expanded/disabled/busy/modal states, a live
  politeness, level, position in set, keyboard shortcut, labelled-by,
  described-by and controls relations, an active descendant, and the messages
  for click, increment, decrement, expand, collapse and set-value.
- **Collection.** `Collect` is an `Operation`: custom reports open a node,
  `text` reports name a control (button caption, check box label) or become
  label children of containers, `focusable` reports give a control its focus
  target and focused state, and scrollable translations are applied to bounds.
  An unnamed slider, pick list, field or switch is named by the text drawn
  just before it and gets a labelled-by relation to it.
- **Publication.** `Bridge` holds one `accesskit_unix` adapter per window (an
  AT-SPI frame), keyed by the window root node's key. Keys keep node IDs
  across updates; generated keys follow their parent's key. Text inputs carry
  a text run for the AT-SPI Text interface; password inputs publish only
  bullets. Focus is the deepest focused node while the window has keyboard
  focus, or the active descendant of a focused input.
- **Requests.** Incoming actions are looked up in the tree last published.
  A retired node has no entry, so stale requests do nothing. Each request
  returns the message ordinary input sends (`Event::Action`), or a focus
  request that moves Iced keyboard focus (`Event::Focus`). Set-value
  functions run on the accessibility thread and reject what the control would.
- **Cost.** `accesskit_unix` connects to the accessibility bus only when
  `org.a11y.Status.IsEnabled` is set. Until an adapter is activated the bridge
  subscribes only to its own channel and `refresh()` returns no task; nothing
  is collected or built. Once active, a collection runs after updates and
  after key presses, clicks, scrolling, resizes and window focus changes, at
  most one at a time.
- **Theme preferences.** `appearance { high-contrast #true }` selects a
  black-and-white palette with 7:1 text, borders and status colors over any
  theme or inline colors. The Settings portal publishes `contrast` and
  `reduced-motion` with `color-scheme` and `accent-color`. A Settings
  Accessibility page edits high contrast, text size and reduced motion in the
  Personalization draft and applies Niri's `cursor { xcursor-size }` at once.
- **Existing integrations.** The lock/greeter input (secret handling, revision
  checks) and Notify's banners (announcement ordering at activation) keep their
  dedicated trees; the Shell provider-panel bridge stays as is. They share the
  same adapter crate and test fixture; moving them onto `Bridge` is optional
  later work and must keep their native tests.

Settings keys are explicit (`settings-button-…`, `settings-page-<id>`) and no
longer built from English labels, so focus and accessibility IDs survive
translation.

## Coverage

- **Settings:** sidebar page list with the current page selected, search
  controlling it and announcing matches, every shared control, live-page
  states (loading, unavailable, applying, failed with Retry) and the new
  Accessibility page.
- **Launcher:** search field with the results list as controlled list and the
  selected row as active descendant, action rows, browse view, error status.
- **Shell:** bar modules, Quick settings cards, power confirmations, menus,
  calendar and tray menus with checked and disabled items, on the shared
  bridge. Provider panels keep their own bridge; the two never share a surface.
- **Files:** every window, toolbar, location bar, tabs, search, sidebar, list,
  column and icon views, dialogs, progress with announcements, Trash,
  Recovery, Quick Look and preferences.
- **Portal:** FileChooser, AppChooser, Access, Background, USB, launcher
  install review, InputCapture and Wallpaper dialogs. Screenshot, ScreenCast,
  GlobalShortcuts, Account and Print are not covered yet.
- **Notify:** unchanged tree; critical banners are announced assertively.

AccessKit exposes only the default action over AT-SPI, so secondary actions
(right click, scroll on a bar module) are not reachable from a screen reader.

## Consequences

- Every Canopa Iced view gets a tree by wrapping its shared controls once;
  pages built from shared helpers are covered without per-page code.
- Semantics depend on views using the wrappers. A raw widget without one is
  still read as text, but has no role or action.
- Native acceptance uses a private D-Bus, `at-spi-bus-launcher` and
  `at-spi2-registryd` (`testing::isolated`), queried over plain
  `org.a11y.atspi` calls. It does not run Orca; the hands-on Orca checks in
  the core-audit reports remain the user-level acceptance.
- Coordinates are window-relative logical pixels times the window's scale
  hint; Wayland gives no global position, so magnifiers follow focus within
  the window.
