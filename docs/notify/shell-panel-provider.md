# Canopa Shell Notification Center Provider

Canopa includes a built-in `notifications` module and Center provider. Place
`"notifications"` in a bar zone or use `open-provider "notifications"`; no provider
block is required for the standard session socket. See the root
[notification guide](../notifications.md) for DND, session setup and restart
behavior. The explicit declarations below remain useful for custom socket paths
and override the built-in provider when the name matches.

Canopa Notify exposes Notification Center content through `panel-provider` mode. The provider talks to the running Notification Daemon through the private runtime socket, streams complete replacement pages of Notification Center content while connected, and returns provider-visible error content if the daemon is unavailable.

Minimal command:

```bash
canopa-notify panel-provider --socket "$XDG_RUNTIME_DIR/canopa-notify/control.sock"
```

The provider emits the shared Canopa panel protocol from `canopa-panel-protocol`, including explicit protocol versioning and complete content replacement messages. **Panel Provider Mode** maps **Notification Center Projection** into generic panel sections, items, controls, metadata, and actions; Notify-specific meaning stays in the provider adapter.

## Compatible Canopa Shell host

The host is the `canopa-shell` member of this Canopa workspace. Configure the new explicit transport; Canopa Shell's default framed-v1 transport is not compatible with this provider:

```kdl
panel-providers {
    provider "notifications" {
        protocol "canopa-v4"
        command "canopa-notify panel-provider --socket \"$XDG_RUNTIME_DIR/canopa-notify/control.sock\""
    }
}
```

Use `click { open-provider "notifications"; }` in a module's `actions` block. If Canopa Shell disables shell commands, use an `argv` command with an explicit absolute socket path instead; argv does not expand `$XDG_RUNTIME_DIR`.

The matching v4 host renders grouped items, controls, metadata, all actions, styled text, bounded local images, and blue underlined links. Keyboard selection uses arrows/Tab, Enter activates the selection, Page Up/Down and Home/End scroll the content, and Escape closes the panel. The selected item/control context stays visible above the scroller. Late actions from older snapshots or sessions are ignored. Native AT-SPI exposes content, images, controls, link roles, geometry, selection and revision-bound activation. Automated native-interface tests and [actual graphical/Orca acceptance](notification-desktop-acceptance.md) cover the supported host.

The streamed Center opens on messages, with delivery rules and history deletion
under **Settings**. Settings applies to apps and categories on the current page;
**Back to notifications** preserves that page. Both views keep pagination controls
when needed. Delivery choices read **Show banners**, **Save quietly**, and **Block**.

Live transient notifications appear in date order alongside retained messages without a retention label, including notifications hidden by banner overflow or DND. All named actions remain reachable. Active items advertise `Open notification` for default activation, plus explicit dismissal; without a default action the item activation control dismisses it. Transient content is never archived and disappears after closure, dismissal, expiry or restart. Opening the Center does not start banner timeouts.

Retained content has a small attention dot until acknowledgement, user removal or sender recall. Its tooltip and accessible name are `Unread`. Ordinary expiry and archive pruning do not remove it. Expired/stale pending items offer `Mark as read`, and acknowledged items have no attention dot. Cards omit retention, lifecycle status, and delivery outcome labels. Sender actions disappear when their lifecycle ends; content links remain available while the retained row exists. See [ADR 0049](adr/0049-retained-attention-and-sender-lifecycle.md).

Upgrade Notify and Canopa Shell together and change the configured transport to `canopa-v4`. Version 3 hosts cannot interpret the new rich-body link variant. The generic link carries its label/style and an opaque provider action; the validated URI is presentation metadata for AT-SPI Hyperlink; Notify alone resolves and opens the command target. Unsupported targets are readable text. Opening a link is independent of sender actions and acknowledgement. The desktop OpenURI portal is required for opening; status reports unavailable/failed requests without exposing URLs.

Item action values are opaque: hosts must return the advertised value unchanged. Each now carries a typed command with a Notification Center Target bound to the displayed revision and daemon lifetime. The daemon rejects queued actions after replacement/termination, even if the action key is reused, and rejects targets from previous daemon lifetimes. Stable panel item IDs do not change on replacement. This complements the host's snapshot-generation checks; it does not add automatic retries or promise that actions are unique on a still-active resident revision. Upgrade daemon and provider together: legacy item-action values without a target fail closed. Ordinary CLI/current-state commands, bulk clear and permission controls keep their existing semantics. See [ADR 0042](adr/0042-revision-bound-notification-center-actions.md).

The canonical wire types live in root `crates/canopa-panel-protocol`. Both applications
use the same path dependency. Run `bash scripts/check-panel-protocol.sh` from the workspace root
to verify that workspace relationship. No source snapshot synchronization is needed.

## Stream limits and recovery

The reading view is one list ordered by creation date descending, with internal ID
as the tie-breaker. App identity and attention state never reorder it. Settings
keeps app-grouped controls and shows the same app icons beside their names.

Cards carry generic source attribution: the app name and its explicit icon, or
the same app-identity icon candidates used by banners. Canopa Shell uses Notify's
configured theme/scale for PNG/JPEG lookup, falls back to an initial badge and
retains the source label for accessibility. Body images remain separate. See
[the compatible protocol extension](../adr/0010-rich-item-source-attribution.md).

Frames include their terminating newline and may not exceed 1 MiB. The installed provider paginates Notification Center content with `First page`, `Previous page` and `Next page` controls. It preserves complete bodies and every action; large action lists can continue on another page. The canonical `rich_body` carries text, images and links, so redundant plain/styled fallback fields are omitted from paginated frames. Navigation belongs to each open panel and rejects stale generations. New content above the current position is reachable with `First page`.

Page construction reads bounded keyset batches and retains one frame plus a candidate row. Each view keeps at most 128 recent previous positions; `First page` remains available beyond that trail. An old stored row that cannot fit even by itself yields an explicit display error; it is never deleted to fit a frame.

At most eight panel subscriptions share the daemon's 64 control connections. Additional panels receive a visible limit error and may be reopened after another closes. The installed provider uses duplex `panel-session`; navigation input is capped at 512 bytes per line, with a five-second partial-frame deadline. Healthy idle panels have no idle timeout, but a socket frame write that stalls for five seconds is disconnected. The legacy one-way `panel-subscribe` interface retains its old single-projection/overflow-error behavior; installed providers use pagination. A disconnected or invalid stream must be reopened; actions are never retried automatically.

Provider input permits 65,518 bytes per newline-terminated interaction, leaving space for the control-command prefix. Oversized or malformed input is discarded without logging its contents, and later valid input remains usable. Each serial interaction exchange has a five-second total deadline and a 4 KiB reply limit. A timeout does not prove that the daemon rejected the action; it may already have committed. Invalid, incompatible or incomplete daemon output becomes a small visible error instead of being forwarded to Canopa Shell. Pipe backpressure adds no application queue, but a host that stops reading stdout must still clean up its provider process.

`body-images` is advertised after the paired renderer checks. `body-hyperlinks` is advertised when the runtime OpenURI opener is available. The [release record](notification-release-verification.md) links the tests and actual desktop evidence behind those claims.

For a taller reading area, set `panel { height 720; }` on the Canopa Shell bar hosting Notifications. The host bounds this preference by the available output and configured limits; automatic provider popovers prefer 600 logical pixels. Page Up/Page Down scroll the content in either configuration.
