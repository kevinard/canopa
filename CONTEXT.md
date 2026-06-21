# Notify

Notify is a desktop notification system for Linux. It receives notifications, decides which ones should be shown, and keeps notification state available to the user.

## Language

**Notification Daemon**:
The long-running owner of desktop notification state. It receives notification requests, applies user policy, displays allowed notifications, and keeps notification records.
_Avoid_: Applet, bar module, plugin

**Canopa Notify**:
The product and installed service identity for this notification system.
_Avoid_: notify when referring to public binary, service, or package identity

**Wayland Session**:
The graphical desktop session in which Notify can present notification banners and notification center content.
_Avoid_: X11 session when referring to supported graphical runtime

**Graphical User Session**:
The single logged-in desktop session that owns Notify's D-Bus name, Wayland connection, runtime socket, and notification state.
_Avoid_: Multi-seat session when referring to first-version support

**Headless Mode**:
A developer/test mode where Notify accepts and processes notifications without creating graphical notification surfaces.
_Avoid_: Server mode, production no-UI mode

**Primary Compositor**:
The compositor used as the main development and validation environment for Notify.
_Avoid_: Required compositor when Notify can run elsewhere

**Compositor Privacy Mode**:
A compositor-owned privacy behavior that may hide or suppress notification content during sensitive display states.
_Avoid_: Notification body hiding when implying Notify owns the behavior

**Notification Center**:
An interactive view of notification state owned by the **Notification Daemon**, including recent and historical notifications.
_Avoid_: Niribar panel when referring to the notification domain, popup

**App Identity**:
The canonical identity of the application that sent a notification, used for per-application policy and history grouping.
_Avoid_: App name when referring to the canonical rule key, process identity

**Notification Rule**:
A user policy that matches notifications and determines whether they are shown, silenced, or dropped.
_Avoid_: Filter when referring to user-visible policy, app permission when the rule can match more than the app

**Config Rule ID**:
An optional stable label for a **Notification Rule** declared in **Notification Configuration**.
_Avoid_: User rule key when referring to configuration-authored rules

**Delivery Outcome**:
The policy result applied to a notification when it arrived.
_Avoid_: Current app permission when referring to historical records

**Loaded Delivery Policy State**:
The normalized, in-memory policy facts evaluated by **Delivery Policy**, including **Do Not Disturb** and **Notification Rules** from all active rule sources.
_Avoid_: Rule store when referring to persisted rules, notification configuration when referring to the normalized state

**Delivery Decision Log**:
A diagnostic record of how Notify classified an incoming notification for delivery policy.
_Avoid_: Metrics when referring to per-notification debug diagnostics

**Notification Content**:
The user-visible summary, body, and other sender-provided human-readable content of a notification.
_Avoid_: Metadata when referring to app identity, urgency, IDs, or delivery outcome

**Notification Intake**:
The daemon-owned normalization of sender notification requests into notification facts used by **Delivery Policy** and **Notification Lifecycle**.
_Avoid_: Notification Server API when referring to daemon-internal normalization

**Accepted Notification**:
The normalized notification shape produced by **Notification Intake** after no-op rejection, **App Identity** derivation, and **Notification Body** normalization, before **Delivery Policy** assigns a **Delivery Outcome**.
_Avoid_: Notification Thread when lifecycle has not created or revised history

**Rule Reference**:
A lightweight reference to the rule source that produced a **Delivery Outcome**.
_Avoid_: Rule snapshot when the full rule definition is not copied into history

**Silenced Notification**:
A notification accepted into history without producing a visible notification surface.
_Avoid_: Blocked notification when the notification is still retained

**Dropped Notification**:
A notification intentionally discarded without being shown or retained in history.
_Avoid_: Silenced notification

**Do Not Disturb**:
A user-controlled global mode that suppresses notification banners without deleting notification state.
_Avoid_: Drop all, mute when the notification should still be retained

**Notification Urgency**:
The sender-declared importance of a notification: low, normal, or critical.
_Avoid_: Priority when referring to the freedesktop notification urgency concept

**Notification Category**:
The sender-declared type of notification, stored as metadata when provided.
_Avoid_: Rule category when notification rules do not match categories

**Notification Progress**:
Sender-provided progress metadata for an in-progress operation represented by a notification.
_Avoid_: Category when referring to operation completion percentage

**Notification Body**:
The user-visible message content of a notification, interpreted from sender-provided body text.
_Avoid_: Raw body when referring to text shown to the user

**Notification Summary**:
The sender-provided short title or headline of a notification.
_Avoid_: Required title when the sender provides an empty summary

**Raw Notification Body**:
The original sender-provided body text retained for protocol fidelity and debugging.
_Avoid_: Notification Body when referring to rendered/plain user-visible text

**Notification Thread**:
A user-visible notification record that retains an **Accepted Notification** plus lifecycle metadata, and may be revised by replacement notifications from the same sender. Normal history shows the latest revision of a thread.
_Avoid_: Notification when referring specifically to the replacement-aware record

**Notification History**:
The persisted collection of retained **Notification Threads** available after their original visible notification surfaces are gone.
_Avoid_: Log when referring to user-visible notification records

**Notification Lifecycle**:
The daemon-owned state changes that create, revise, or move a **Notification Thread** between active, dismissed, expired, closed, and stale states and produce the corresponding **Sender Notification Lifecycle** effects.
_Avoid_: Storage status update when referring to user-visible or sender-visible lifecycle meaning

**Lifecycle Effect**:
A **Notification Lifecycle** result that tells the **Notification Daemon** what sender-visible or presentation-visible side effects to publish after lifecycle state changes.
_Avoid_: Sender event when referring to the lifecycle module's returned instruction rather than the published event

**Active Notification**:
A retained **Notification Thread** that has not yet been dismissed, expired, or closed by the sender.
_Avoid_: Historical notification when the notification is still pending user attention

**Notification Banner**:
The transient on-screen presentation of an **Active Notification** outside the **Notification Center**.
_Avoid_: Popup when referring to Notify's user-visible notification presentation

**Dismissed Notification**:
A retained **Notification Thread** that is no longer active because the user dismissed it.
_Avoid_: Deleted notification, cleared notification

**Expired Notification**:
A retained **Notification Thread** that is no longer active because its visible notification surface timed out.
_Avoid_: Dismissed notification when the user did not dismiss it

**Closed Notification**:
A retained **Notification Thread** that is no longer active because the sender closed it.
_Avoid_: Dismissed notification when the sender closed it

**Stale Notification**:
A retained **Notification Thread** that was active before the **Notification Daemon** restarted and is no longer connected to a live sender lifecycle.
_Avoid_: Active notification after daemon restart

**Clear Active Notifications**:
A user action that dismisses retained **Active Notifications** without deleting them from **Notification History**.
_Avoid_: Delete history, clear history

**Delete Notification History**:
A user action that removes retained historical notification records.
_Avoid_: Clear active notifications

**Notification Action**:
A sender-provided action the user may invoke from an **Active Notification**.
_Avoid_: Button when referring to the sender callback rather than its visual control

**Default Notification Action**:
The sender-provided action invoked when the user activates the notification itself rather than a named action control.
_Avoid_: Open notification center

**Resident Notification**:
A notification whose sender requests that it remain active after action invocation.
_Avoid_: Persistent notification when referring specifically to post-action behavior

**Notification Timeout**:
The configured or sender-requested duration after which an active notification may expire.
_Avoid_: Retention when referring to how long history is kept

**App Permission Control**:
A **Notification Center** control that changes the **Notification Rule** for one **App Identity**.
_Avoid_: Global rule editor when the control only affects one app

**Notify Control API**:
Notify's private per-user interface for inspecting and changing **Notification Daemon** state, including notification center, history, rules, and command-line control.
_Avoid_: Notification server API when referring to freedesktop notification receipt

**Notification State Subscription**:
A **Notify Control API** stream that delivers notification state changes to a connected client.
_Avoid_: Polling when referring to push updates

**Notification Center Projection**:
A derived current-state representation of **Notification Center** content produced from **Notification Daemon** state, including grouped **Active Notifications**, **Notification History**, and **App Permission Control** state.
_Avoid_: Event log when referring to display state, Notification Snapshot when referring to display-ready content

**Panel Provider Mode**:
The `canopa-notify` mode that serves Notify-owned notification center content to Niribar.
_Avoid_: Notification Daemon when referring to the short-lived provider process

**Control Command**:
A command-line operation that inspects or changes Notify state through the **Notify Control API**.
_Avoid_: Notification action when referring to user/admin control of Notify itself

**Notification Server API**:
The freedesktop D-Bus interface used by applications to submit and close desktop notifications.
_Avoid_: Notify Control API

**Sender Notification Lifecycle**:
The sender-visible lifecycle of a notification after it has been submitted to the **Notification Server API**.
_Avoid_: Notification History state when referring to what the sender observes

**Notification Configuration**:
The user-owned settings that define notification rules, presentation defaults, and history retention.
_Avoid_: Runtime state, notification history

**User Rule Store**:
The daemon-owned persisted source of notification rules changed through Notify's own UI or control commands.
_Avoid_: Notification configuration when referring to UI-managed policy

## Rules

- A **Notification Rule** may silence a notification without dropping it.
- A **Notification Rule** may match an **App Identity** and optionally **Notification Urgency**.
- An explicit drop **Notification Rule** applies even to critical notifications.
- **Notification Category** is stored as metadata but has no first-version category-specific UI.
- **Notification Progress** is stored as metadata and may be rendered by **Notification Banners**.
- Notify stores **Raw Notification Body** while displaying a conservative **Notification Body** representation.
- The first **Notification Body** rendering is plain text.
- **Notification Intake** owns first-version **App Identity** derivation and **Notification Body** normalization.
- **Notification Intake** returns an **Accepted Notification** for sender requests that should enter **Delivery Policy** and **Notification Lifecycle**.
- **Accepted Notification** contains normalized sender/request facts, not lifecycle timestamps.
- Body-only notifications are valid even when **Notification Summary** is empty.
- Notifications with no **Notification Summary**, **Notification Body**, **Notification Action**, or meaningful metadata are dropped as no-op notifications.
- **Notification Intake** validates sender input and rejects no-op notifications before **Delivery Policy**.
- No-op **Notification Intake** rejection has the same sender-visible **Delivery Outcome** as a **Dropped Notification**.
- No-op **Notification Intake** rejection returns directly from intake; it does not enter **Delivery Policy** or **Notification Lifecycle**.
- An **Accepted Notification** may carry the sender-provided replacement target, but interpreting replacement behavior belongs to **Notification Lifecycle**, not **Notification Intake**.
- **Do Not Disturb** silences non-critical notifications while still allowing critical **Notification Banners** by default.
- **Do Not Disturb** state persists across **Notification Daemon** restarts.
- **Do Not Disturb** takes precedence over app allow rules for non-critical notifications.
- **Do Not Disturb** is part of loaded **Delivery Policy** rule state.
- **Loaded Delivery Policy State** is assembled outside **Delivery Policy** from the **User Rule Store**, **Notification Configuration**, and persisted **Do Not Disturb** state.
- The **Delivery Policy** module may provide the builder for **Loaded Delivery Policy State**, but the **Notification Daemon** supplies already-loaded rule and **Do Not Disturb** facts.
- A **Silenced Notification** remains visible in notification history.
- Unknown **App Identities** are allowed unless a **Notification Rule** says otherwise.
- Notification delivery policy uses ordered **Notification Rules** where the first decisive matching rule determines whether a notification is shown, silenced, or dropped.
- **Delivery Policy** runs before **Notification Lifecycle** for both new and replacement notifications.
- **Delivery Policy** decides the **Delivery Outcome**; **Notification Lifecycle** acts on that outcome, including dropped notifications.
- **Delivery Policy** evaluates an immutable **Accepted Notification** and **Loaded Delivery Policy State**; it does not query storage or configuration files directly.
- First-version **Delivery Policy** rules use **App Identity** and **Notification Urgency** from the **Accepted Notification**, but the module interface accepts the full shape so future rules can use additional normalized facts.
- **Delivery Policy** uses the same decision path for new notifications, replacement notifications, and **Active Notification** re-evaluation unless a future **Notification Rule** explicitly depends on that context.
- Default allowing of unknown **App Identities** is **Delivery Policy** implementation, not a synthetic rule supplied by the **Notification Daemon**.
- **Delivery Policy** produces the diagnostic explanation used for a **Delivery Decision Log**; the **Notification Daemon** records the log without inferring policy reasons.
- **Delivery Policy** precedence and diagnostic behavior are tested through the **Delivery Policy** module interface; **Notification Daemon** tests cover rule loading, re-evaluation, and **Notification Lifecycle** wiring.
- **Delivery Decision Logs** are diagnostic debug logs, not first-version metrics.
- Normal **Delivery Decision Logs** do not include **Notification Content**.
- Replacement notifications revise an existing **Notification Thread** instead of creating a new normal history item.
- Replacement handling belongs to **Notification Lifecycle**, not **Notification Intake** or **Notification History**.
- Revising a **Notification Thread** replaces its retained **Accepted Notification** facts while preserving thread identity and lifecycle metadata.
- **Active Notification** re-evaluation uses the **Accepted Notification** retained by each **Notification Thread** instead of reconstructing a separate policy input shape.
- **Notification Lifecycle** owns lifecycle timestamps on **Notification Threads**.
- **Notification Lifecycle** may use an internal clock seam so lifecycle timestamp behavior is testable without moving timestamp ownership into **Notification History**.
- **Delivery Outcome** and **Rule Reference** are **Notification Thread** lifecycle metadata, not **Accepted Notification** facts.
- Notification delivery policy applies to replacement notifications; a silenced replacement updates history and closes any visible **Notification Banner** for the thread.
- A dropped replacement is handled by **Notification Lifecycle** without revising the existing **Notification Thread**.
- An eligible replacement notification may create a **Notification Banner** even when the previous revision did not.
- A dropped replacement notification does not update **Notification History**.
- A replacement notification whose **Notification Timeout** makes expiration ineligible explicitly cancels any pending expiration for the revised **Notification Thread**.
- Re-evaluating an **Active Notification** from shown to silenced or dropped explicitly cancels any pending **Notification Timeout** expiration for that **Notification Thread**.
- A **Dropped Notification** is immediately closed from the **Sender Notification Lifecycle** perspective.
- A **Silenced Notification** may remain an **Active Notification** without a visible **Notification Banner**.
- **Notification History** persists across **Notification Daemon** restarts by default.
- Default **Notification History** retention keeps up to 30 days and 1,000 **Notification Threads**, with either limit allowed to remove older history.
- **Notification History** retention is pruned after every persisted **Notification History** mutation and at daemon startup.
- **Notification History** retention pruning is persistence policy owned by **Notification History**, not **Notification Lifecycle** meaning.
- **Active Notifications** are exempt from **Notification History** retention pruning.
- **Notification History** does not store **Notification Banner** runtime visibility state.
- **Notification Daemon** restart does not restore old **Notification Banners**.
- **Active Notifications** loaded after **Notification Daemon** restart become **Stale Notifications**.
- Marking restart-loaded **Active Notifications** as **Stale Notifications** is **Notification Lifecycle** meaning; **Notification History** may query active threads but must not decide the stale thread shape.
- Dismissing an **Active Notification** makes it a **Dismissed Notification** without removing it from **Notification History**.
- User dismissal has the same **Sender Notification Lifecycle** effect whether it starts from a **Notification Banner** or the **Notification Center**.
- Expiring an **Active Notification** makes it an **Expired Notification** without removing it from **Notification History**.
- Sender-closing an **Active Notification** makes it a **Closed Notification** without removing it from **Notification History**.
- Moving an **Active Notification** to dismissed, expired, closed, or stale explicitly cancels any pending **Notification Timeout** expiration for that **Notification Thread**.
- Critical **Active Notifications** do not expire automatically by default.
- First-version **Notification Timeout** configuration is global with per-urgency defaults, not per-app.
- Sender-requested **Notification Timeout** values are respected within user-configured bounds.
- **Notification Banners** appear at the top right of the focused output by default.
- **Notification Banners** are owned by the **Notification Daemon**, not by Niribar.
- Notify shows at most three **Notification Banners** at once by default.
- **Notification Banner** visibility prefers critical **Active Notifications**, then newest eligible **Active Notifications**.
- Hidden **Active Notifications** are not later promoted to **Notification Banners** solely because a banner slot opens.
- **Notification Banner State** consumes **Lifecycle Effects** and decides banner eligibility itself.
- **Notification Lifecycle** does not emit banner-specific effects.
- **Notification Banner** visibility is not derived from persisted **Notification History** flags.
- **Notification Banner** state is runtime-only and is not persisted in **Notification History**.
- Activating a **Notification Banner** invokes its **Default Notification Action** when present and otherwise dismisses the notification.
- The first banner placement scope is global rather than per-output.
- If focused output cannot be determined, **Notification Banners** fall back to a configured or default output.
- First-version **Notification Banner** styling configuration is minimal.
- First-version **Notification Banner** keyboard interaction is minimal.
- **Notification Banners** do not steal keyboard focus.
- Notification sounds are outside the first version scope.
- Hiding notification body content for privacy-sensitive display states belongs to **Compositor Privacy Mode**, not Notify.
- Notify supports **Wayland Sessions** only.
- The first version supports one **Graphical User Session** per user.
- The **Notification Daemon** enforces a single active instance per **Graphical User Session**.
- The **Notification Daemon** is a coordinating owner over separate deep modules, not one monolithic module that owns every implementation detail directly.
- Separate **Notification Daemon** modules should be deep enough that their interfaces concentrate lifecycle, policy, history, or presentation rules instead of leaking those rules across callers.
- **Notification Lifecycle** is separate from **Notification History**; lifecycle rules command persistence changes rather than being owned by storage.
- **Notification Lifecycle** returns **Lifecycle Effects**; the **Notification Daemon** publishes sender-visible events and presentation updates from those effects.
- **Lifecycle Effects** are the primary **Notification Lifecycle** result; the **Notification Daemon** derives sender receipts and outward side effects from them.
- A **Lifecycle Effect** may include the sender receipt for the accepted or rejected notification request.
- **Lifecycle Effects** include **Notification Timeout** scheduling and cancellation instructions; timer adapters perform waiting but do not decide timeout meaning.
- **Lifecycle Effects** are fine-grained events rather than one coarse outcome struct.
- **Notification Lifecycle** owns the domain shape of created and revised **Notification Threads**; **Notification History** persists that shape.
- **Notification Lifecycle** may query **Notification History** directly for existing **Notification Threads**, including replacement targets.
- **Notification Lifecycle** writes **Notification History** directly when applying lifecycle transitions.
- **Notification History** is a storage engine behind a trait interface for **Notification Lifecycle**.
- **Notification History** adapters persist complete **Notification Thread** changes requested by **Notification Lifecycle**; they do not infer lifecycle transitions such as replacement revision increments, active resets, or closed timestamp resets.
- **Notification History** adapters may enforce generic persistence preconditions, such as expected **Notification Thread** revision or status, to prevent stale writes without deciding lifecycle meaning.
- **Notification Lifecycle** writes to existing **Notification Threads** should use a persistence precondition by default; unconditional history writes are reserved for explicit storage maintenance or repair paths.
- **Notification History** adapters may allocate storage-owned **Notification Thread** identifiers and return the persisted **Notification Thread** shape, but lifecycle metadata in that returned shape must come from **Notification Lifecycle**.
- After **Notification Thread** creation, the **Notification Lifecycle** to **Notification History** seam should use a generic persist operation for complete thread changes rather than lifecycle-specific persistence methods such as revise, transition, or update delivery.
- **Notification History** adapters store and return shared notification domain objects, not storage-specific records for other modules to adapt.
- **Notification Lifecycle** tests may use an in-memory **Notification History** adapter, but that adapter should model persistence semantics only and must not duplicate lifecycle transition rules.
- The **Notification Lifecycle** to **Notification History** seam is async.
- The first **Notification Lifecycle** module is a generic struct over the **Notification History** trait, not a trait-object module.
- Shared notification domain objects live inside the **Notification Daemon** crate for now; a separate domain crate is premature.
- **Headless Mode** exists for development and tests, not as a normal product mode.
- Outside **Headless Mode**, failure to connect to a **Wayland Session** is a startup failure.
- Niri is Notify's **Primary Compositor**, not its only supported compositor.
- The **Notification Daemon** fails fast when it cannot own the **Notification Server API**.
- The first **Notification Server API** scope is freedesktop compatibility, not Dunst compatibility.
- Dunst configuration import is outside the first version scope.
- **Notification Server API** identity reports product name **Canopa Notify** and vendor Canopa.
- **Panel Provider Mode** reports a provider-visible error when the **Notification Daemon** is unavailable instead of starting it.
- **Panel Provider Mode** uses a **Notification State Subscription** while the **Notification Center** is open.
- A **Notification State Subscription** is a current-state stream, not an event log.
- The first **Notification State Subscription** sends complete replacement **Notification Center Projections**, not patches.
- **Notification State Subscription** does not expose **Lifecycle Effects**; each emitted **Notification Center Projection** replaces the previous display state.
- Slow **Notification State Subscription** clients may skip intermediate **Notification Center Projections** because each projection replaces previous display state.
- **Notification Center Projections** are rebuilt from **Notification Daemon** state when emitted, not maintained as separate source state.
- **Notification Center Projection** rebuilds on request; it does not consume **Lifecycle Effects** as state.
- Notify has one active **Notification Configuration** at a time.
- Reloading invalid **Notification Configuration** keeps the previous valid configuration active.
- Valid **Notification Configuration** reloads re-evaluate **Active Notifications** under the new rules.
- **Notification Daemon** and configuration errors are reported through logs and control surfaces, not self-notifications.
- The first version includes minimal **Control Commands** for daemon status, active notifications, **Do Not Disturb**, and configuration reload.
- First-version **Control Commands** are diagnostic-first and do not promise stable output formats unless explicitly documented.
- Data-returning **Control Commands** may support unstable JSON output for diagnostics and tests.
- **Control Commands** report when the **Notification Daemon** is unavailable instead of starting it implicitly.
- The **Notification Daemon** owns notification state, lifecycle, policy, history, and rules; the **Notification Center** is an interactive view over that daemon state.
- The **Notify Control API** exposes all private control surfaces over **Notification Daemon** state, including diagnostic status, active notification output, **Control Commands**, **Notification Center Projection**, and **Notification State Subscription**.
- Diagnostic **Control Commands** are adapters over the **Notify Control API** module, not a separate control path.
- The **Notify Control API** returns domain-shaped replies; command-line text, JSON, socket frames, and panel protocol messages are adapter formatting.
- Pre-release control command formats do not require legacy compatibility.
- User interface changes to **Notification Rules** are saved in the **User Rule Store**, not by rewriting **Notification Configuration**.
- **User Rule Store** rules are keyed by **App Identity** and optional **Notification Urgency**.
- **Notification Configuration** rules may define a **Config Rule ID**; unnamed config rules use generated references.
- **Notification Rules** from the **User Rule Store** take precedence over rules from **Notification Configuration**.
- Changes to **Notification Rules** affect future notifications and do not retroactively delete or reclassify **Notification History**.
- Changes to **Notification Rules** re-evaluate **Active Notifications** without changing inactive **Notification History** records.
- Rule re-evaluation may close existing **Notification Banners** but does not create late banners for older active notifications.
- **Notification History** may store a **Rule Reference** for the rule that produced a historical **Delivery Outcome**.
- **Notification History** does not persist arbitrary notification image payloads by default.
- **Notification History** is not app-level encrypted in the first version.
- The **Notification Center** shows both **Active Notifications** and recent **Notification History**.
- The **Notification Center** groups notifications by **App Identity**.
- The **Notification Center** includes **App Permission Controls** for recent notification senders.
- **App Permission Controls** show current policy; historical records may show their original **Delivery Outcome**.
- **Notification Center** requirements must not define core **Notification Daemon** state or lifecycle rules.
- **Notification Center Projection** is Notify-owned state and is mapped to shared panel protocol types by **Panel Provider Mode**.
- The shared panel protocol contains generic Niribar panel infrastructure, not Notify-specific notification logic.
- The first Niribar-hosted **Notification Center** does not require progress rendering.
- First-version documentation includes a Niribar configuration example for opening the **Notification Center**.
- The first **Notification Center** does not show counts for **Dropped Notifications**.
- Full-text **Notification History** search is outside the first **Notification Center** scope.
- Unread/read notification state is outside the first version scope.
- A standalone settings UI is outside the first version scope.
- A standalone tray or status indicator is outside the first version scope.
- Explicit localization infrastructure is outside the first version scope.
- Opening the **Notification Center** does not change notification state.
- The first **Notification Center** must be keyboard navigable.
- An open **Notification Center** updates live as notification state changes.
- **Clear Active Notifications** and **Delete Notification History** are separate actions.
- **Notification Actions** are invokable only while their **Notification Thread** is active.
- **Notification Actions** remain invokable for silenced **Active Notifications** shown in the **Notification Center**.
- Inactive **Notification History** is read-only except for deletion.
- **Stale Notification** action labels may remain as read-only history metadata.
- Invoking a **Notification Action** closes the notification unless it is a **Resident Notification**.
- **Notification Timeout** scheduling and expiration belong to **Notification Lifecycle**, not **Notification Banner** presentation.
- Inline replies are outside the first version scope.
