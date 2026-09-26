# Module Actions Design

**Date:** 2026-04-26
**Author:** Kevin

## Summary

Add shared module action configuration so any config-declared module can run a user command on left click or right click. Actions live in an `actions` child block on each module. When configured, the action for that specific interaction overrides the module's built-in behavior.

## Goals

- Let every config-declared module define left-click and right-click shell actions.
- Keep the config shape shared across module types.
- Preserve all existing built-in module interactions when no matching configured action exists.
- Make configured actions override built-in behavior for the same interaction.
- Keep action parsing and dispatch independently testable.

## Non-Goals

- Do not add global action defaults.
- Do not add double-click, middle-click, hover, or keyboard-only action config in this change.
- Do not capture command output or surface command failures in the UI.
- Do not change scroll interactions.
- Do not remove the existing button module; its current `on-click` remains compatible.

## Config Syntax

Each module may include an optional `actions` block:

```kdl
modules {
    module "clock" {
        actions {
            click "gnome-clocks"
            right-click "notify-send clock"
        }
    }

    module "volume" {
        step 5
        actions {
            click "pavucontrol"
            right-click "wpctl set-mute @DEFAULT_AUDIO_SINK@ toggle"
        }
    }
}
```

`click` and `right-click` each take one non-empty string argument. The string is executed as a shell command, matching the current button `on-click` behavior.

For the existing button module, both forms are valid:

```kdl
module "button" {
    label "Terminal"
    icon ">"
    on-click "kitty"
}

module "button" {
    label "Terminal"
    icon ">"
    actions {
        click "kitty"
        right-click "notify-send Terminal"
    }
}
```

For button modules, either legacy `on-click` or `actions.click` is required. This keeps existing button configs valid while allowing new button configs to use the shared action block only. If both legacy `on-click` and `actions.click` exist, `actions.click` wins because shared configured actions have precedence.

## Shared Types

Add a reusable config model in `src/config.rs`:

```rust
pub struct ModuleActionsConfig {
    pub click: Option<String>,
    pub right_click: Option<String>,
}
```

`ModuleConfig` gains:

```rust
pub actions: ModuleActionsConfig
```

The default is empty, preserving current behavior for existing configs.

## Parser Design

Add a shared parser helper for `actions` blocks under `src/config/parse/`. The module-section parser reads this block once for every module, independent of module type, and stores it on `ModuleConfig`.

Validation:

- Missing `actions` block is allowed.
- Empty `actions` block is allowed and behaves like no configured actions.
- Every child inside `actions` must be named `click` or `right-click`.
- Each action node must have exactly one non-empty string argument.
- Duplicate `click` or duplicate `right-click` nodes are errors.

Error paths follow current style, for example:

- `modules[0].clock.actions.click`
- `modules[1].volume.actions.right-click`
- `modules[2].script.actions[0]`

## Runtime Design

The central module interaction dispatch checks configured actions before module built-ins:

1. Look up the module's configured action for the incoming interaction.
2. If an action exists, run it with `spawn_command_fire_and_forget(CommandSpec::Shell(...))` and stop.
3. If no configured action exists, call the module's existing `on_interaction` handler.

This preserves built-in behavior for modules such as volume scrolling, workspace focus, tray icon activation, and tray icon menus unless the user configures an action for the same interaction.

Right-click becomes a first-class module runtime event, parallel to the existing left-click and scroll events. The shared module render wrapper emits it for ordinary modules. Tray icons currently install their own right-click mouse area for menu requests, so their rendering must check shared actions too: if a `right-click` action exists on the source `tray` module config, the tray icon runs that action instead of opening the menu.

## Dynamic Modules

Dynamic modules inherit actions from their config placeholder:

- `tray_icon` modules inherit actions from the `tray` module declaration that produced them.
- `workspace_item` modules inherit actions from the `workspaces` module declaration that produced them.

This keeps action configuration attached to the user-visible module declaration instead of requiring users to configure runtime-only module names.

## Button Compatibility

`ButtonModule` keeps its current legacy `on-click` implementation as the built-in fallback. The shared action layer sits above it:

- `actions.click` configured: run shared action.
- `actions.click` missing: call `ButtonModule::on_interaction(Clicked)`, which runs legacy `on-click`.
- `actions.right-click` configured: run shared action.
- `actions.right-click` missing: no right-click behavior.

## Testing

Parser tests:

- Parses `actions.click`.
- Parses `actions.right-click`.
- Allows missing and empty `actions`.
- Rejects unknown action nodes.
- Rejects missing, empty, non-string, and extra action arguments.
- Rejects duplicate action nodes.

Runtime tests:

- Configured left-click action runs before the module built-in.
- Configured right-click action runs before the module built-in.
- Missing configured action falls back to built-in behavior.
- Button legacy `on-click` still works without `actions.click`.
- `actions.click` overrides button legacy `on-click`.
- Tray icon right-click uses inherited `tray.actions.right-click` instead of opening the menu.

## Compatibility

Existing configs behave unchanged because the default action config is empty. The default bundled config can document `actions` as commented examples without changing default behavior.
