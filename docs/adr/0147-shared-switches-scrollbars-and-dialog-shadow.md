# 0147 Shared switches, scrollbars and dialog shadow

Status: accepted. Builds on [ADR 0003](0003-shared-appearance.md) and
[ADR 0146](0146-window-shape-and-dialog-elevation.md). Files' use is described
in [Files ADR 0171](../files/adr/0171-menus-tabs-views-and-secondary-windows.md).

## Decision

- `canopa_ui::scrollbar` is a quiet scrollbar for any `scrollable`: no track,
  a slim rounded thumb in the text colour that firms up under the pointer and
  while dragged. Iced's default track and thumb read as heavy controls.
- `canopa_ui::switch_style` styles Iced's toggler as an on/off switch: the
  accent track when on, a neutral track when off, a light knob in both.
- `canopa_ui::checkbox_style` leaves an empty box transparent, with an edge
  leaning toward the text, so it reads on cards in both schemes instead of as a
  dark hole.
- `packaging/niri/defaults.kdl` gives floating windows a lighter shadow
  (softness 20, offset 6, 25% black); with the hairline border it separates
  dialogs without a heavy halo on light surfaces.

## Consequences

Applications adopt the scrollbar and switch styles where they call them; the
checkbox change applies to every caller of `checkbox_style`. The compositor
shadow applies to every floating window in the supported session.
