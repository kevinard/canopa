# Notification backend preparation

Status: implemented and advertised after scoped native/activation acceptance;
final shared validation in progress, 2026-10-29.

## Primary contracts

The [backend contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.Notification.html)
uses application-scoped string IDs for Add/Remove, version/SupportedOptions
properties, and `ActionInvoked(app_id, id, action, parameter: av)`. The parameter
array carries an optional target followed by platform data containing the
activation token; there is no separate fifth platform-data signal argument.

The [public contract](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Notification.html)
requires notifications to outlast the sending application. Exported `app.`
actions use application activation; other actions use the signal. Version 2
includes descriptor-backed icons/sounds, markup bodies and optional category/
button-purpose capabilities. V2 backend icons do not use the deprecated bytes
form. Descriptor resource ownership and supported-option reporting therefore
need explicit implementation and tests.

## Existing Canopa ownership

Notify owns Delivery Policy, Notification Lifecycle, Notification History,
Notification Banners, Notification Center, sound and input-bound activation.
The portal adapter should preserve those owners and their current Iced surfaces.
The [domain context](../../docs/notify/CONTEXT.md) distinguishes App Identity,
Internal Notification ID and the live Sender Notification ID. The latter is a
session-global u32 registration, retired on sender lifecycle closure; it is not a
history row ID and is not a portal notification's string ID.

`crates/canopa-notify/src/notification_server.rs` already publishes ordered
ActivationToken, ActionInvoked and NotificationClosed events. Its intake accepts
the reserved `default` action with an empty label for portal-style body activation.
`notification_server/ids.rs` owns allocation and retirement. A bridge through this
API is a candidate, but its ownership/restart/resource behavior must be resolved
before choosing it over a Notify-owned additional adapter.

## Required state and seams

- Maintain application + portal-ID identity independently from live sender IDs.
  Serialize replacement/removal for each identity, and handle retired numeric IDs
  without closing or invoking a different notification after reuse.
- Keep notifications independent from the dialog Requests pool and caller-loss
  cancellation. Sending application exit does not withdraw them.
- Pin service ownership/generations across Notify restart. Cover early method-
  reply signals, replacement/action races and orderly backend teardown. Establish
  what mapping/target state must survive a portal backend restart; retained Notify
  history alone cannot reconstruct the adapter's callback registration.
- Preserve typed action targets and ordered activation platform data. The backend
  performs exported application activation. Keep protocol action keys separate
  from user-visible labels and from Notify-owned Center controls.
- Prepare bounded immutable descriptor resources with explicit lifetime through
  asynchronous presentation. Retire temporary resources when their owning
  notification/revision ends. Handle invalid/unsealed/oversized descriptors and
  the older icon representation deliberately, using existing asset decoders.
- Preserve plain-body meaning and declared markup meaning through Notify intake.
  Translate priority/category/sound/display hints under Notify's policy, and
  report only supported category/purpose capabilities.

## Verification before advertisement

Use model and private-bus tests for complete signatures, ID isolation, replacement,
withdrawal, typed targets, icon/sound descriptor limits, service restart and races.
A real frontend fixture should use disposable application registrations and a
recording activation executable. Native Notify banner/Center interactions must
exercise default/named actions and token propagation in both appearances; verify
application-exit survival without launching real user applications.

The consolidated workspace's existing red outcomes are documented independently
in `.scratch/test-consolidation/VALIDATION.md`: ten Files integration failures and
two host AT-SPI fixture failures. Preserve that baseline evidence when evaluating
new portal changes. Do not infer a new portal failure or a green workspace from
unrelated scoped results.


## Chosen integration seam

[ADR0031](../../docs/adr/0031-notify-owned-portal-adapter.md) selects a Notify-owned
additional adapter and descriptor. Source inspection confirmed that the owned
receipt hook and ordered sender publisher execute under one mutation coordinator.
A frozen portal binding can therefore retain typed targets through replacement
without changing the core action event into a portal-specific DTO. The separate
freedesktop bridge candidate is superseded by this decision. The adapter, native
Center handoff and shipped metadata now pass scoped acceptance. Final shared
validation is in progress; exact results remain in the implementation ledger.


## Activation probe correction

The private real-frontend probe in `/tmp/canopa-notification-wire-02` confirms
that action targets preserve their original types: `(si)` for the default target,
`s` for a button. Byte icons arrive as one sealed descriptor (WRITE/GROW/SHRINK;
SEAL_SEAL is not required). Probe03 confirms no default action is synthesized.
Probe04/05 timed out waiting for an exported callback from an emitted backend
signal; neither is an activation pass. Source inspection of GNOME Shell's
`GtkNotificationDaemonNotification._activateAction` and the GNOME portal adapter
shows exported actions call org.freedesktop.Application directly; the portal
signal serves non-exported actions. The Canopa adapter must therefore implement
Activate/ActivateAction itself and test its callback/platform-data separately.
The earlier claim that the frontend owns exported activation is corrected.


## Implemented first slice

Notify now hosts the interface at the standard path with a readiness-ordered
service alias. Earlier focused verification passed seven adapter checks, all 38
binary checks, 26 intake checks, six raster/SVG checks and 276 Notify integration
cases. The subsequent generic activation handoff passes 13 protocol cases and
the focused Shell callback-replacement and Notify captured-action regressions.
Six native checks per appearance exercise the real public frontend, sender exit,
typed exported/non-exported actions, native activation consumption, withdrawal
and Center source/serial-bound context. Both appearances also pass private
activation through the shipped descriptor/service files. Resources
become owned pixels/encoded sound bytes; explicit body formats preserve plain
text and exclude unsupported portal markup. Callbacks do not expire automatically;
first-version banners remain until explicit action/dismissal/withdrawal. See the
current evidence ledger for pending full consumer, shared and core validation.
