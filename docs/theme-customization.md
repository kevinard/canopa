# Theme and UI customization

Shell and Notify share the `appearance` block in the schema-1 Canopa configuration.
The built-in Canopa dark and light themes follow the [desktop visual direction](adr/0014-canopa-visual-direction.md).
Try [the native desktop example](../examples/canopa-desktop.kdl), which selects the
bundled spine layout. Change its `color-scheme` to `"light"` for the companion theme.
Start with [the complete example](../examples/custom-theme.kdl) and its
[Nord theme](../examples/themes/nord.kdl). Validate with
`canopa config check --file examples/custom-theme.kdl` before installing.

```kdl
appearance {
    theme "themes/nord.kdl"
    palette { accent "#a3be8c"; }
    typography {
        family "Inter" "Noto Sans" "sans-serif"
        body-size 14
        label-size 12
        title-size 18
    }
    ui {
        surface { border width=1 color="border"; radius 10; }
        control { radius 6; }
        bar { padding 4 8; gap 6; }
        panel { padding 18; gap 10; }
        button { state "hover" { background "raised"; }; }
    }
}
```

A theme file contains the **contents** of `appearance`, without that wrapper or an
application namespace. Theme files cannot include other themes. Relative paths are
resolved from the main configuration's directory; absolute paths are supported.
Both files must be regular UTF-8 files, with a combined limit of 1 MiB. Missing or
invalid selected themes reject the candidate instead of silently switching presets.

Each property resolves independently: built-in preset, theme file, inline appearance,
then the element's local `ui` block. In each appearance layer, `surface` and `control`
defaults apply before specific kinds, regardless of source order. Omitting a property
inherits it; zero explicitly removes spacing, radius or border width. There is no
reset keyword, selector language, or arbitrary CSS.

## Colors and fonts

`color-scheme "dark"` (default) or `"light"` supplies omitted colors. `palette`
accepts six-digit `#rrggbb` values for `background`, `surface`, `raised`, `text`,
`muted`, `disabled`, `border`, `accent`, `danger`, `warning`, `success`, `on-accent`,
`on-danger`, `focus`, `spine`, `accent-surface`, and `brand`. `surface` is the panel
color, `spine` the navigation frame, and `accent-surface` a quiet selected fill.
`brand` is secondary identity color, not status. Filled accent/danger text chooses a contrasting default;
focus defaults to accent. Custom palettes are not automatically contrast-corrected.

The legacy `accent "#rrggbb"` setting remains an alias. It cannot be combined with
`palette { accent ...; }` in the same file layer. Existing shell color token aliases
(`base`, `surface0`, `surface1`, `subtext`, `overlay0`, `critical`) remain accepted.
UI colors accept role names, `#rrggbb`, `#rrggbbaa`, or `"transparent"`.

`family` is an ordered list of up to eight installed font family names, followed by
an optional generic family (`sans-serif`, `serif`, `monospace`, `cursive`, `fantasy`).
The first available family is selected; otherwise the system default is used. This
list chooses a family; the text engine still owns per-glyph fallback. Font files are
not loaded from configuration. Symbol icons retain their dedicated symbol family.
`body-size`, `label-size`, and `title-size` are positive logical pixels, at most 256.
Existing explicit module/segment text sizes, weights and colors remain supported.

In the main configuration's `appearance` block, `text-scale 125` applies a
125% multiplier to the resolved typography sizes. The supported range is 100
to 150, with a default of 100. `reduced-motion #true` disables Shell panel
transitions. Canopa Settings exposes both options on the Personalization page.

## UI properties

All dimensions are logical pixels, with up to three decimal places of precision;
Iced applies output scaling. Sizes are finite and nonnegative. Spacing and radius
are capped at 4096, and border width at 64.

| Property | Values |
| --- | --- |
| `background`, `foreground` | One color or role name |
| `border` | `width=N`, `color="role-or-color"`, or both |
| `radius` | One value, or top-left / top-right / bottom-right / bottom-left |
| `padding` | One value, vertical/horizontal, or top/right/bottom/left |
| `margin` | Same forms as padding; adjacent margins add together |
| `gap` | One value between immediate children owned by the layout |

The kinds are `bar`, `panel`, `menu`, `tooltip`, `notification`, `card`, `module`,
`button`, `input`, and `menu-row`. `surface` covers the first six; `control` covers
the last four. Groups support colors, border, radius and padding. Kind-specific
gap is supported on bar, panel, menu, notification and card. Margin is supported
on panel, menu, tooltip, notification, card, module and menu-row. Bar placement
margins belong on the bar itself. Buttons and text inputs use their parent's gap
and padding instead of an individual margin setting. Inapplicable settings fail
validation.

A frame's border is drawn inside its bounds. Explicit borders reserve content
space. Popover/tooltip margins are transparent space included in surface measurement;
banner margins participate in the banner stack. Notify's screen offset remains
Notify-owned. Radius is limited by available frame dimensions. Existing attached
bar separators and square drawers remain unless a border/radius is configured;
the bundled spine explicitly selects rounded, borderless bar and panel frames.

Supported color-only states are:

| Kind | States |
| --- | --- |
| `module` | `selected`, `hover`, `focus` |
| `button` | `hover`, `pressed`, `disabled`, `focus` |
| `input` | `hover`, `disabled`, `focus` |
| `menu-row` | `selected`, `hover`, `disabled`, `focus` |

Use `state "hover" { background "raised"; foreground "text"; border-color "accent"; }`.
Applicable states compose in selected, hover, pressed, disabled, focus order. Focus
applies only to enabled controls. Layout cannot change with state. Semantic status
colors and explicit formatted segments keep their role rather than being recolored
as ordinary labels. Theme configuration never changes action identity or focus ownership.

## Local overrides and bar placement

Local `ui` blocks are accepted on `shell.bar`, a bar's `panel`, named
`shell.modules.module`, named `shell.menus.menu`, and `notifications.banners`.
For example:

```kdl
shell {
    bar "main" {
        edge "top"
        thickness 44
        length "85%"
        align "center"
        margin 10 16 6 16
        reserve-space #true
        ui { radius 12; border width=1 color="accent"; padding 4 10; }
        panel { ui { padding 20; }; }
        modules { center "clock"; }
    }
    modules { module "clock" { ui { background "surface"; padding 2 8; }; }; }
}
notifications { banners { ui { margin 4 0; }; }; }
```

A bar's `panel { title "System controls"; }` names only its default Bar Panel.
Omitting `title` retains the Overview title. Menus and module details keep their
content-specific titles. Titles must be nonempty strings.

Use `open-bar-panel` or `toggle-bar-panel` in a module's action to open its current
bar's configured panel, retaining the module as the anchor and focus return target.
The existing `open-panel` and `toggle-panel` actions continue to open that module's
own details. Bar background actions already target the Bar Panel.

`thickness` is the entire visible bar frame, including padding and border. Its legacy
alias `collapsed-width` remains supported; specifying both is an error. Thickness
is an integer from 2 to 32768 and must leave content space after explicit border and
padding. `length` is `"fill"` (default), a positive integer up to 32768, or a quoted
percentage from `"0.01%"` to `"100%"`. Percentages use the available edge span after
margins. `align` is `"start"` (default), `"center"`, or `"end"`; start is left on
horizontal edges and top on vertical edges.

Bar placement margins round upward to whole logical pixels for native geometry.
Other UI spacing retains fractional logical precision. Margins use screen directions.
On a top bar, top separates the visible frame from the usable edge, left/right reduce
the available span, and bottom separates it from
application content. `reserve-space #true` (default) reserves near margin + thickness
+ inner margin across the edge, including for a short bar. With `#false`, application
windows may extend beneath it; panels and tooltips still open beyond the frame.
Other reserved surfaces may move the usable edge inward.

The native host spans the edge with a transparent background. Painting and the
compositor input region use one visible-frame geometry; unused host areas and
rounded corners pass clicks through. Fixed lengths clamp to the available span.
If edge margins consume that span, start margin is fitted first, then end margin,
leaving at least one logical pixel. Content zones clip to their available space;
large configured fonts do not increase the configured bar thickness.

## Reload and provenance

Shell watches the main file and selected theme's metadata, including creation,
deletion, symlink target changes and atomic replacement. A valid candidate installs
palette, typography, layout and geometry together. Invalid edits retain the previous
complete appearance. Notify and the Settings portal retain their own explicit reload
lifetimes; changing a file does not synchronize all processes automatically.

`source.fingerprint` still identifies the main file's bytes. `source.dependencies`
records the selected theme path and fingerprint, and `source.combined_fingerprint`
identifies both inputs. The combined validator captures both files once for its
consumer validations, so concurrent edits cannot give the consumers different theme
contents. Errors identify theme provenance without echoing configuration values.
