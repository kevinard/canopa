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

Live transient notifications now appear in their application group with `Retention: Runtime only`, including notifications hidden by banner overflow or DND. All named actions are included. Active items also advertise an `Open notification` control for default activation, or `Dismiss notification` when there is no default action; inactive history exposes neither. These are provider-owned item controls, not additional sender-named actions. Transient content is never archived and disappears after closure, dismissal, expiry or daemon restart. Opening the center does not start banner timeouts. `Retention: Retained` distinguishes normal history-backed items. The underlying Canopa-v3 protocol is unchanged; actual graphical/keyboard acceptance remains on the [verification checklist](notification-center-verification.md).

The canonical wire types remain in `crates/canopa-panel-protocol`; Niribar vendors the same source so it can build without a sibling checkout. After protocol edits, synchronize the host snapshot and run:

```bash
bash scripts/check-panel-protocol.sh /path/to/niribar
```

## Stream limits and recovery

Frames include their terminating newline and may not exceed 1 MiB. If a complete Notification Center projection exceeds that display budget, Notify emits an error instead of partial content. History and active notifications are unchanged; use the control interfaces to inspect/clear retained state. The existing subscription resumes normal content when a subsequent projection fits. Pagination and aggregate projection-building memory remain unfinished.

At most eight panel subscriptions may share the daemon's 64 control connections. Additional panels receive a visible limit error and may be reopened after another panel closes. A subscription is one-way after `panel-subscribe`: closing its write half or sending additional bytes ends it. Healthy idle panels have no idle timeout, but a socket frame write that stalls for five seconds is disconnected. A disconnected or invalid provider stream must be reopened; there is no automatic action retry or reconnect in this change.

Provider input permits 65,518 bytes per newline-terminated interaction, leaving space for the control-command prefix. Oversized or malformed input is discarded without logging its contents, and later valid input remains usable. Each serial interaction exchange has a five-second total deadline and a 4 KiB reply limit. A timeout does not prove that the daemon rejected the action; it may already have committed. Invalid, incompatible or incomplete daemon output becomes a small visible error instead of being forwarded to Niribar. Pipe backpressure adds no application queue, but a host that stops reading stdout must still clean up its provider process.

`body-images` remains unadvertised until a graphical check confirms both banners and this host. Codec/image tests do not substitute for that check. The full remaining completion work is tracked in `.scratch/freedesktop-completion-review/IMPLEMENTATION.md`.
