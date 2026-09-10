# Niribar Notification Center Provider

Canopa Notify exposes Notification Center content through `panel-provider` mode. The provider talks to the running Notification Daemon through the private runtime socket, streams complete replacement Notification Center content while connected, and returns provider-visible error content if the daemon is unavailable.

Minimal command:

```bash
canopa-notify panel-provider --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock"
```

The provider emits the shared Canopa panel protocol from `canopa-panel-protocol`, including explicit protocol versioning and complete content replacement messages. **Panel Provider Mode** maps **Notification Center Projection** into generic panel sections, items, controls, metadata, and actions; Notify-specific meaning stays in the provider adapter.

## Compatible Niribar host

The host port targets `/home/kevinard/projects/niribar`. Configure the new explicit transport; Niribar's default framed-v1 transport is not compatible with this provider:

```kdl
panel-providers {
    provider "notifications" {
        protocol "canopa-v3"
        command "canopa-notify panel-provider --socket \"$XDG_RUNTIME_DIR/canopa-notify/control.sock\""
    }
}
```

Use `click { open-provider "notifications"; }` in a module's `actions` block. If Niribar disables shell commands, use an `argv` command with an explicit absolute socket path instead; argv does not expand `$XDG_RUNTIME_DIR`.

The port renders grouped items, controls, metadata, all actions, styled text and bounded local images. Keyboard selection uses arrows/Tab, Enter activates the selected action and Escape closes the panel. The selected item/control context stays visible above the scroller. Late actions from older snapshots or sessions are ignored. Screen-reader integration and hyperlinks remain separate work.

The canonical wire types remain in `crates/canopa-panel-protocol`; Niribar vendors the same source so it can build without a sibling checkout. After protocol edits, synchronize the host snapshot and run:

```bash
bash scripts/check-panel-protocol.sh /path/to/niribar
```

`body-images` remains unadvertised until a graphical check confirms both banners and this host. Codec/image tests do not substitute for that check. The full remaining completion work is tracked in `.scratch/freedesktop-completion-review/IMPLEMENTATION.md`.
