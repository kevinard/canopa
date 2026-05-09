Status: ready-for-human

# Add Configurable Bar Layer

## What to build

Expose **Layer** as a configurable property of each Bar. Bar Surfaces use the Layer configured by their Bar, while Menus and Tooltips keep popup behavior and do not inherit the Bar layer.

## Acceptance criteria

- [ ] Bar config supports `background`, `bottom`, `top`, and `overlay` layer values.
- [ ] The default Bar Layer is `top`.
- [ ] Each Bar Surface uses the Layer configured by its Bar.
- [ ] Menus and Tooltips continue to use popup/overlay behavior rather than inheriting the Bar Layer.
- [ ] Invalid layer values produce actionable configuration errors.
- [ ] Tests cover default layer, all accepted values, and invalid values.

## Blocked by

None - can start immediately
