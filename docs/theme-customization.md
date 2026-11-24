# Theme customization

The `appearance` block of `~/.config/canopa/config.kdl` sets colors, fonts, text
size, contrast, motion and element styling for the whole desktop. Every Canopa
application reads it: Shell (bars, panels, launcher, on-screen display), Notify
(banners), Files, Settings, Wallpaper, the lock screen, the login screen, the Polkit
and Pinentry dialogs, and the Settings portal, which passes the color scheme,
accent, contrast and reduced-motion preference to other applications.

Canopa Settings edits the common options on its **Personalization** and
**Accessibility** pages. Everything below can also be written by hand.

## Built-in themes

`color-scheme` selects one of two built-in palettes:

| Role | `"dark"` (default) | `"light"` |
| --- | --- | --- |
| `background` | `#10191d` | `#e3eae6` |
| `surface` | `#202d31` | `#fbfcf8` |
| `raised` | `#2b3b3f` | `#e7ede7` |
| `text` | `#edf4f0` | `#20342c` |
| `muted` | `#b5c7c3` | `#52675e` |
| `disabled` | `#778c88` | `#788980` |
| `border` | `#647d76` | `#889b90` |
| `accent` | `#b0dec3` (mint) | `#256948` (forest green) |
| `danger` | `#f38ba8` | `#b01843` |
| `warning` | `#f9e2af` | `#855900` |
| `success` | `#a6e3a1` | `#326a1f` |

The other roles (`spine`, `accent-surface`, `brand`, `on-accent`, `on-danger` and
`focus`) are derived from these. `focus` defaults to the accent, and text on accent
or danger fills picks a readable color automatically. The built-in palettes meet
WCAG contrast for text; custom colors are not corrected.

```kdl
appearance {
    color-scheme "light"
    accent "#3b6ea5"
}
```

To try the native look, run a Shell with the bundled layout and your appearance:
`canopa-shell --canopa-config examples/canopa-desktop.kdl` from a source checkout
(the examples are installed in `/usr/share/canopa/examples/`).

## The appearance block

| Key | Value | Default | Theme file |
| --- | --- | --- | --- |
| `theme "PATH"` | theme file, relative to `config.kdl` or absolute | none | no |
| `color-scheme` | `"dark"` or `"light"` | `"dark"` | yes |
| `accent "#rrggbb"` | accent color | scheme accent | yes |
| `palette { ROLE "#rrggbb"; }` | any color role | scheme colors | yes |
| `typography { … }` | fonts and sizes | see below | yes |
| `text-scale N` | integer percentage, 100 to 150 | 100 | no |
| `high-contrast` | `#true` / `#false` | `#false` | no |
| `reduced-motion` | `#true` / `#false` | `#false` | no |
| `ui { … }` | element styling | built-in | yes |

Each key may appear once. Colors in `palette` and `accent` are opaque six-digit
`#rrggbb` values.

### Palette roles

`background`, `surface` (panels), `raised` (controls), `text`, `muted`, `disabled`,
`border`, `accent`, `danger`, `warning`, `success`, `on-accent`, `on-danger`,
`focus`, `spine` (the bar frame), `accent-surface` (a quiet selected fill) and
`brand` (a secondary identity color, not a status color).

These aliases are also accepted: `base` (background), `surface0` (surface),
`surface1` (raised), `subtext` (muted), `overlay0` (disabled), `critical` (danger).

`accent "#rrggbb"` is shorthand for `palette { accent "#rrggbb"; }`. A layer cannot
use both. An inline `accent` replaces a theme file's palette accent.

### Typography

```kdl
typography {
    family "Inter" "Noto Sans" "sans-serif"
    body-size 14
    label-size 12
    title-size 20
}
```

`family` lists up to eight installed font families, in order of preference, and may
end with a generic family (`sans-serif`, `serif`, `monospace`, `cursive`,
`fantasy`). The first available family is used, and the system default otherwise.
Fonts are not loaded from files. Sizes are logical pixels, greater than 0 and at
most 256. The defaults are `"Inter" "sans-serif"`, 14, 12 and 20.

`text-scale 125` multiplies all three sizes by 125%.

### Contrast and motion

`high-contrast #true` replaces the palette with black and white surfaces,
full-strength text and borders, and keeps the accent and status colors only when
they reach 7:1 against the background (otherwise it substitutes high-contrast
colors). It applies over theme-file and inline colors.

`reduced-motion #true` turns off panel transitions and the on-screen display fade,
and is published to other applications through the Settings portal. The compositor's
animations are separate: Settings turns them off in the Niri configuration.

## Theme files

A theme file holds the contents of an `appearance` block, without the
`appearance { }` wrapper:

```kdl
// themes/nord.kdl
color-scheme "dark"
palette {
    background "#2e3440"
    surface "#3b4252"
    text "#eceff4"
    accent "#88c0d0"
}
typography { family "Inter" "Noto Sans" "sans-serif"; }
```

```kdl
// config.kdl
schema-version 1
appearance {
    theme "themes/nord.kdl"
    palette { accent "#a3be8c"; }
}
```

- A theme file cannot set `theme`, `text-scale`, `high-contrast` or
  `reduced-motion`, and cannot include another theme.
- The path is relative to the directory of `config.kdl`; absolute paths work.
- A missing or invalid theme file is an error (reported with source `theme`), and
  applications keep their previous appearance. There is no silent fallback.
- The main file and theme file together are limited to 1 MiB.

Each property resolves in order: built-in palette, theme file, inline `appearance`,
then an element's own `ui` block. Omitted properties are inherited; `0` explicitly
removes spacing, radius or border width.

Samples: [Nord theme](../examples/themes/nord.kdl) and
[a complete configuration using it](../examples/custom-theme.kdl) (installed under
`/usr/share/canopa/examples/`). Validate a candidate before installing it:

```sh
canopa config check --file examples/custom-theme.kdl
```

## UI styling

`ui` styles element kinds:

```kdl
ui {
    surface { border width=1 color="border"; radius 10; }
    control { radius 6; }
    bar { padding 4 8; gap 6; }
    panel { padding 18; gap 10; }
    button { state "hover" { background "raised"; }; }
}
```

Kinds: `bar`, `panel`, `menu`, `tooltip`, `notification`, `card` (the surface
group) and `module`, `button`, `input`, `menu-row` (the control group). `surface`
and `control` set defaults for their group and apply before the specific kinds,
whatever their order in the file.

| Property | Value | Applies to |
| --- | --- | --- |
| `background`, `foreground` | role name, `#rrggbb`, `#rrggbbaa` or `"transparent"` | all |
| `border` | `width=N` (up to 64), `color="…"`, or both | all |
| `radius` | one value, or top-left top-right bottom-right bottom-left | all |
| `padding` | 1, 2 (vertical horizontal) or 4 (top right bottom left) values | all |
| `margin` | same forms as padding | `panel`, `menu`, `tooltip`, `notification`, `card`, `module`, `menu-row` |
| `gap` | one value between children | `bar`, `panel`, `menu`, `notification`, `card` |
| `state "NAME" { background; foreground; border-color; }` | state colors | control kinds |

Groups accept colors, border, radius and padding only. Dimensions are logical
pixels with up to three decimals, at most 4096. Settings that do not apply to a kind
fail validation.

States per control kind:

| Kind | States |
| --- | --- |
| `module` | `selected`, `hover`, `focus` |
| `button` | `hover`, `pressed`, `disabled`, `focus` |
| `input` | `hover`, `disabled`, `focus` |
| `menu-row` | `selected`, `hover`, `disabled`, `focus` |

States change colors only, never layout. They combine in the order selected, hover,
pressed, disabled, focus; focus applies only to enabled controls.

Built-in defaults:

| Kind | Padding | Radius | Border | Background | Gap |
| --- | --- | --- | --- | --- | --- |
| `bar` | 4 | 0 | 0 | `background` | 4 |
| `panel` | 24 | 24 | 1 | `surface` | 8 |
| `menu` | 8 | 16 | 1 | `surface` | 4 |
| `tooltip` | 8 12 | 12 | 1 | `raised` | |
| `notification` | 16 | 24 | 1 | `surface` | 8 |
| `card` | 16 | 16 | 1 | `raised` | 8 |
| `module` | 2 | 12 | 0 | transparent | |
| `button` | 8 12 | 12 | 1 | `raised` | |
| `input` | 10 12 | 12 | 1 | `raised` | |
| `menu-row` | 10 12 | 12 | 0 | transparent | |

Borders are drawn inside an element's bounds. Margins on popovers and tooltips are
transparent space around them; banner margins space the banner stack.

The portal file chooser uses the Files panel and input styles for its browser
surface, toolbar and location and search fields. Other portal dialogs
follow the effective palette and the compositor's titlebar style.

## Local overrides

A `ui` block can also be placed on a single element. It applies after the shared
`appearance`:

| Where | Kind |
| --- | --- |
| `shell { bar "NAME" { ui { … } } }` | `bar` |
| `shell { bar "NAME" { panel { ui { … } } } }` | `panel` |
| `shell { modules { module "NAME" { ui { … } } } }` | `module` |
| `shell { menus { menu "NAME" { ui { … } } } }` | `menu` |
| `notifications { banners { ui { … } } }` | `notification` |

```kdl
shell {
    bar "main" {
        edge "top"
        thickness 44
        length "85%"
        margin 10 16 6 16
        ui { radius 12; border width=1 color="accent"; padding 4 10; }
        panel { ui { padding 20; }; }
        modules { center "clock"; }
    }
    modules { module "clock" { ui { background "surface"; padding 2 8; }; }; }
}
notifications { banners { ui { margin 4 0; }; }; }
```

## Bar placement

Bar geometry is part of the `shell` block (see the [configuration
reference](configuration.md#bar)):

- `thickness` is the whole visible frame, including padding and border, from 2 to
  32768 logical pixels. It must leave room for content after padding and border.
- `length` is `"fill"` (default), a pixel count, or a percentage from `"0.01%"` to
  `"100%"` of the edge span left after margins.
- `align` is `"start"`, `"center"` (default) or `"end"`. Start is left on horizontal
  bars and top on vertical bars.
- `margin` uses screen directions. On a top bar, the top margin separates the bar
  from the screen edge, left and right shorten the span, and bottom separates it from
  windows. Bar margins round up to whole pixels.
- `reserve-space #true` (default) keeps windows clear of margin, thickness and inner
  margin, even for a short bar. With `#false`, windows can extend beneath the bar.

Areas of the bar's edge outside the visible frame, and rounded corners, let clicks
through. Large fonts do not grow the bar; content is clipped to its zone.

## Reload

| Consumer | When appearance changes apply |
| --- | --- |
| Shell, Files, Wallpaper | Automatically after a save, including edits to the theme file. |
| Notify | After `canopa-notify ctl reload`; open banners repaint. |
| Settings portal | After `systemctl --user reload xdg-desktop-portal-canopa.service`. |
| Lock screen, Pinentry | The next time they open. |
| Polkit agent, login screen | When they start. |

When Canopa Settings saves appearance, it also reloads Notify and the Settings
portal. An invalid change keeps the previous complete appearance.

`canopa config check` reports the theme file in each component's
`source.dependencies` with its own fingerprint, and `source.combined_fingerprint`
covers both files.

## Icons

The icon theme is not part of `appearance`. Canopa follows `gtk-icon-theme-name` in
the `[Settings]` group of GTK's `settings.ini` (`gtk-4.0` before `gtk-3.0`, in
`$XDG_CONFIG_HOME` and then `$XDG_CONFIG_DIRS`), falling back to hicolor. Notify can
pin a theme with `notifications { icons theme="…"; }`.

## Not supported

Themes deliberately do not offer arbitrary CSS or selectors, expressions,
recursive inheritance, remote fonts or themes, a graphical editor, content-sized
bars, gradients, blur, shadow customization or animations, and they do not theme
other applications or Niri's window decorations. Per-placement module style
overrides are not available yet.
