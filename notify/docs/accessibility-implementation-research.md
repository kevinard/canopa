# Accessibility implementation research

Research date: 2026-09-16. Scope: a real AT-SPI bridge for Notify banners and the Niribar Notification Center, using the existing Iced 0.14 and vendored iced_layershell renderer. This note proposes implementation; it does not claim that the graphical surfaces already pass screen-reader acceptance.

## Recommended seam

Use the maintained **AccessKit Unix adapter directly**, with one adapter per live layer-shell surface. It takes a semantic tree and user-action callbacks without requiring a winit window. A toolkit-wide Iced migration is therefore unnecessary for these two bounded surfaces. AccessKit implements AT-SPI using Rust and zbus; its producer sends tree updates and receives requested actions. [AccessKit architecture](https://github.com/AccessKit/accesskit)

The existing application projections already contain the required content:

- Notify: `BannerUiState`, `BannerPresentation`, `BannerView`, `BannerTarget`, and `BannerUiMessage` in [presentation.rs](../crates/canopa-notify/src/presentation.rs). The surface reducer supplies actual creation, replacement, and retirement boundaries.
- Niribar: `CanopaSnapshot`, `CanopaSession`, and `action_id` in its `src/external_panel/canopa.rs`; rendering in `src/app/canopa_panel.rs`; revision-specific action widgets in `src/app/canopa_panel/action_widget.rs`.
- The [panel protocol](../crates/canopa-panel-protocol/src/lib.rs) provides titles, ordered text/image bodies, image alternatives, metadata, controls, action labels, and opaque interaction targets.

These are local-source observations. Niribar was inspected read-only; this note makes no edits there.

## Verified dependencies and APIs

The published dependency pair is:

```toml
accesskit = "0.25.0"
accesskit_unix = { version = "0.23.0", default-features = false, features = ["tokio"] }
```

Unix 0.23.0 requires Rust 1.85, AccessKit 0.25.0, `accesskit_atspi_common` 0.20.0, `atspi` 0.29 with `proxies`, and zbus 5.5. Its own default `async-io` and `tokio` features are mutually exclusive. The underlying zbus dependency still enables `async-io`; do not interpret that transitive feature as conflicting with the adapter's own mutually exclusive features. These facts were checked against the downloaded published manifest and source, not inferred from older release examples. [Published manifest](https://docs.rs/crate/accesskit_unix/0.23.0/source/Cargo.toml), [feature checks](https://docs.rs/crate/accesskit_unix/0.23.0/source/src/lib.rs)

`Adapter::new(activation_handler, action_handler, deactivation_handler)` accepts `Send + 'static` implementations of the three AccessKit handler traits. Its callbacks run on another thread. Updates use `update_if_active(|| update)`; native focus uses `update_window_focus_state(bool)`. Dropping the adapter removes that surface. `set_root_window_bounds` is intended for X11; Wayland does not provide global window positions. [Adapter API](https://docs.rs/accesskit_unix/0.23.0/accesskit_unix/struct.Adapter.html)

The Unix adapter creates a dedicated background thread, and its Tokio configuration creates a current-thread Tokio runtime there. It does not require borrowing Notify's runtime to construct the adapter. It watches **`org.a11y.Status.IsEnabled`**, not `ScreenReaderEnabled`, and activates/deactivates adapters when accessibility is toggled. Its session connection and message channel are process globals, so integration fixtures must run in separate processes when using different private buses. [Context implementation](https://docs.rs/crate/accesskit_unix/0.23.0/source/src/context.rs), [runtime implementation](https://docs.rs/crate/accesskit_unix/0.23.0/source/src/util.rs)

Minimal tree construction for this exact schema:

```rust
use accesskit::{Action, Node, NodeId, Role, TreeId, TreeInfo, TreeUpdate};

let root_id = NodeId(1);
let action_id = NodeId(2);
let mut root = Node::new(Role::Window);
root.set_label("Notifications");
root.set_children([action_id]);
let mut action = Node::new(Role::Button);
action.set_label("Open conversation");
action.add_action(Action::Click);
let update = TreeUpdate {
    nodes: vec![(root_id, root), (action_id, action)],
    tree: Some(TreeInfo::new(root_id)),
    tree_id: TreeId::ROOT,
    focus: root_id,
};
```

`Tree` is a deprecated alias of `TreeInfo`. `ActionRequest` contains `action`, `target_tree`, `target_node`, and `data`. Match the root tree and supported action before dispatching. Use `Focus` only where the application implements corresponding focus behavior. [Node API](https://docs.rs/accesskit/0.25.0/accesskit/struct.Node.html), [action request](https://docs.rs/accesskit/0.25.0/accesskit/struct.ActionRequest.html)

Every update includes the current focus node; use the root when no child has keyboard focus. Initial updates include all nodes and `TreeInfo`. Nodes are removed by updating their parent's children and omitting the removed subtree. Supplying the complete current tree is valid initially; incremental updates reduce processing for large Center histories. [Tree update contract](https://docs.rs/accesskit/0.25.0/accesskit/struct.TreeUpdate.html)

## Application design

The following is an implementation recommendation based on the local ownership and interaction model.

1. Add a small accessibility module owning an adapter, its latest complete semantic snapshot, a bounded action mapping, and a sender back to the UI loop. Activation synchronously clones the latest complete snapshot. Do not call the adapter while holding the shared snapshot lock: activation and updates otherwise risk opposite lock ordering.
2. Create the adapter when the actual surface exists, publish content that corresponds to that surface, update it on replacement and focus changes, and drop it when the surface closes. Do not expose unseen history through the banner adapter or leave stale Center content after closing.
3. Give each surface a stable `Window` root. Group each notification's app name, summary, ordered body, metadata, and actions together. Use `Label` for readable text, `Image` with the sender's alternative text, and `Button` for actionable labels. Expose link nodes with `Role::Link`, a label, a URL, and the same validated opening behavior as the visual link. Decorative app icons do not need duplicate names. Missing images preserve the same alternative/fallback text as the screen.
4. Give every action its localized sender label even when the visual control contains an icon. Distinguish repeated action labels using the notification/section context. Preserve button labels in full; visual ellipsis is not a reason to truncate accessible names.
5. Allocate monotonic `NodeId`s. Preserve identity for the same semantic object while its layout changes. Action identities include the complete banner target/revision or Niribar generation-scoped `action_id`; allocate new IDs when those change. Retire inactive mappings without reusing old IDs. Do not derive action identity from display index, label, notification number alone, or a hash with unchecked collisions.
6. In the callback, resolve the node to its original exact action payload and enqueue it. Notify should use `ActivateRequested`, `InvokeActionRequested`, or `Dismiss`, preserving activation-token handling and existing stale-target rejection. Niribar should use `ExternalPanelEvent::ButtonActivated` with the existing opaque `element_id`; `CanopaSession::write_interaction` already rejects stale generations. Revalidate on the UI/runtime side because content can change after the callback reads its map.
7. Center focus requests select the same action as keyboard navigation and scroll it into view. Publish focus only after the application changes it. Keep banners non-focusing unless an explicit user interaction requests a supported focus transition.
8. Feed actual layout rectangles and scroll offsets from wrapper widgets into semantic node bounds. `ActionWidget` and Notify's `LayoutFrame` measurement pattern are suitable existing seams. Do not invent desktop coordinates on Wayland. Labels and actions without geometry can establish initial protocol coverage, but do not establish complete Orca spatial-review acceptance.

Names, descriptions, roles, and parent/child references are part of the public AT-SPI `Accessible` interface; actionable controls also expose `Action`, whose `DoAction` invokes the requested affordance. [Accessible interface](https://gnome.pages.gitlab.gnome.org/at-spi2-core/devel-docs/doc-org.a11y.atspi.Accessible.html), [Action interface](https://gnome.pages.gitlab.gnome.org/at-spi2-core/devel-docs/doc-org.a11y.atspi.Action.html)

## Announcements

Use one notification announcement node with `set_live(Live::Polite)` and a spoken label combining app, summary, and ordered body alternatives. Set `Live::Off` on its content/action descendants to avoid inherited duplicate announcements. Keep that label stable across layout, image decoding, and unrelated snapshots. The common adapter emits `Announcement` when a live node is inserted and when its accessible name changes. This supports arrival and meaningful replacement without stealing focus. [Insertion handling](https://docs.rs/crate/accesskit_atspi_common/0.20.0/source/src/adapter.rs), [property-change handling](https://docs.rs/crate/accesskit_atspi_common/0.20.0/source/src/node.rs)

The proposed policy is to announce arriving visible banners politely and let users navigate Center history without announcing the entire backlog. Avoid setting the entire Center to live. Test late activation with a visible banner, replacement, identical refresh, and retirement explicitly. Critical urgency does not require asserting focus; any interrupting speech policy should be deliberate and separately verified.

## Executable verification

Observed on this machine: `gdbus`, Python GI `Atspi`, `/usr/lib/at-spi-bus-launcher`, and `/usr/lib/at-spi2-registryd` are installed. `orca` is absent. AccessKit was absent from the Cargo source cache at the beginning of research; the parent implementation fetched the 0.25.0/0.23.0 sources during this work. The local compiler is Rust 1.97 nightly.

The following private-bus setup was exercised through the installed public AT-SPI client and returned `desktop frame`:

```bash
dbus-run-session -- bash -c '
set -eu
atspi_fixture_dir=$(mktemp -d /tmp/canopa-atspi-fixture.XXXXXX)
export XDG_RUNTIME_DIR="$atspi_fixture_dir/runtime"
export XDG_CONFIG_HOME="$atspi_fixture_dir/config"
export DCONF_BACKEND=memory
mkdir -m 700 "$XDG_RUNTIME_DIR" "$XDG_CONFIG_HOME"
/usr/lib/at-spi-bus-launcher --launch-immediately --a11y=1 &
launcher=$!
trap "kill $launcher 2>/dev/null || true" EXIT
for attempt in $(seq 1 50); do
  if gdbus call --session --dest org.a11y.Bus \
    --object-path /org/a11y/bus --method org.a11y.Bus.GetAddress \
    2>/dev/null; then break; fi
  sleep 0.05
done
/usr/lib/at-spi2-registryd &
registry=$!
trap "kill $registry $launcher 2>/dev/null || true" EXIT
# Start the test application and public AT-SPI assertions here.
'
```

Fresh runtime/config directories matter: a private session bus alone does not isolate the AT-SPI socket. Explicit registry startup matters on this machine because dbus-broker's registry activation otherwise expects a systemd user manager that the private session lacks. Retain fixture-owned directories until helper processes have terminated; the production harness should clean its temporary directory automatically.

In the same environment, Python can traverse `Atspi.get_desktop(0)`, locate the test application, and recursively inspect `get_child_count`, `get_child_at_index`, `get_name`, and `get_role_name`. A Rust integration test can alternatively use the project's zbus connection with `org.a11y.Bus.GetAddress`, then query the public `Accessible` and `Action` interfaces on that address. Test a production semantic adapter without a graphical compositor first, then repeat against actual mapped surfaces.

Required assertions: full summary/body/alternative text; localized action names and roles; every sender action; correct `DoAction(0)` dispatch; stale action reference rejected after same-ID replacement; retired nodes defunct; duplicate refresh creates no extra announcement; arrival creates a polite announcement; focus changes follow real keyboard navigation; close/reopen does not expose retired controls; accessibility toggling returns the latest tree. Register announcement/focus listeners before mutation and use bounded waits rather than relying on arbitrary sleeps.

Finally run Orca in a supported real compositor session with speech enabled. Verify announcement intelligibility, ordered reading, full action context, focus and scrolling, image alternatives, link activation, replacement, and dismissal. Orca's flat review is spatial, so geometry must be checked as well as names. This manual pass remains necessary: successful D-Bus introspection is not evidence that the spoken interaction is usable. [Orca introduction and invocation](https://help.gnome.org/orca/introduction.html), [flat-review commands](https://help.gnome.org/users/orca/stable/commands_flat_review.html.en)
