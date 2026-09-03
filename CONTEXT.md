# Notify

Notify is a desktop notification system for Linux. It receives notifications, decides which ones should be shown, and keeps notification state available to the user.

## Language

**Notification Daemon**:
The long-running owner of desktop notification state. It receives notification requests, applies user policy, displays allowed notifications, and keeps notification records.
_Avoid_: Applet, bar module, plugin

**Notification Runtime**:
The stateful core inside the **Notification Daemon** that owns loaded policy state, **Notification Lifecycle**, **Notification History** access, **User Rule Store** changes, **Do Not Disturb**, **Lifecycle Effect** application, timers, **Notification Banner State**, sender-visible lifecycle publication, and **Notification State Subscription** publication.
_Avoid_: Tokio runtime, edge adapter, daemon shell

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

**Notification Server Request**:
The raw `Notify` method values received through the **Notification Server API** before they are decoded into **Notification Intake** input.
_Avoid_: Accepted Notification when the request has not passed through Notification Intake

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

**Transient Notification**:
A sender-requested notification that participates in runtime presentation and **Sender Notification Lifecycle** without entering persisted **Notification History** or the first **Notification Center**.
_Avoid_: Dropped notification, short-timeout notification

**Do Not Disturb**:
A user-controlled global mode that suppresses notification banners without deleting notification state.
_Avoid_: Drop all, mute when the notification should still be retained

**Notification Urgency**:
The sender-declared importance of a notification: low, normal, or critical.
_Avoid_: Priority when referring to the freedesktop notification urgency concept

**Notification Category**:
The sender-declared type of notification, stored as metadata when provided.
_Avoid_: App Identity, Notification Urgency

**Notification Progress**:
Sender-provided progress metadata for an in-progress operation represented by a notification.
_Avoid_: Category when referring to operation completion percentage

**Notification App Icon**:
A visual identifier for the application that sent a notification, distinct from any visual content attached to the notification itself.
_Avoid_: Notification Image when referring to the sending application's visual identity

**Notification Image**:
Sender-provided visual content attached to a specific notification.
_Avoid_: Notification App Icon when referring to notification-specific visual content

**Body-Embedded Notification Image**:
Sender-provided visual content referenced from **Notification Body Markup** as part of the notification's detailed message content.
_Avoid_: Notification Image when referring to notification-level visual hints, inline image when the body relationship is unclear

**Notification Body Segment**:
An ordered part of interpreted **Notification Body** presentation, such as styled text or a **Body-Embedded Notification Image**, while the **Notification Body** remains the plain-text fallback.
_Avoid_: Body text run when referring to non-text body content, image list when ordering matters

**Notification Body**:
The user-visible message content of a notification, interpreted from sender-provided body text.
_Avoid_: Raw body when referring to text shown to the user

**Notification Body Markup**:
Sender-provided lightweight presentation hints in a **Raw Notification Body** that may affect how the **Notification Body** is shown without changing its plain-text meaning.
_Avoid_: HTML, rich text, hyperlink support when the link is not actionable

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

**Notification Timeout Change**:
A **Lifecycle Effect** projection that tells the **Notification Daemon** timer adapter to start a **Notification Timeout** when the matching revision is displayed, or to cancel an existing timeout.
_Avoid_: Timeout event when referring to the adapter instruction rather than the lifecycle meaning that produced it

**Banner Lifecycle Change**:
A **Lifecycle Effect** projection consumed by **Notification Banner State** to consider a **Notification Thread** for banner visibility or remove a visible **Notification Banner**.
_Avoid_: Lifecycle Effect when referring only to the banner-specific projection

**Active Notification**:
A retained **Notification Thread** that has not yet been dismissed, expired, or closed by the sender.
_Avoid_: Historical notification when the notification is still pending user attention

**Notification Banner**:
The transient on-screen presentation of an **Active Notification** outside the **Notification Center**.
_Avoid_: Popup when referring to Notify's user-visible notification presentation

**Notification Sound**:
A transient audible cue associated with a notification being shown to the user in a **Graphical User Session**. It is part of notification presentation, not retained notification history, and is resolved from local audio resources.
_Avoid_: Alert sound, history sound

**Default Notification Sound**:
The **Notification Sound** used when a shown notification does not request a specific sound.
_Avoid_: Fallback file, system bell

**Notification Sound Theme**:
The named desktop sound theme used to resolve sender-provided sound names and the **Default Notification Sound**.
_Avoid_: Audio backend, media player

**Notification Sound Suppression**:
A sender request that no **Notification Sound** be played for a notification. It does not by itself silence, drop, or hide the notification.
_Avoid_: Silenced Notification, Do Not Disturb

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

**Activation Token**:
A sender-visible activation grant associated with a user interaction on an **Active Notification**, emitted before the sender is told which action was invoked.
_Avoid_: Focus request, action token

**Resident Notification**:
A notification whose sender requests that it remain active after action invocation.
_Avoid_: Persistent notification when referring specifically to post-action behavior

**Notification Timeout**:
The configured or sender-requested duration counted from the first display of the current revision in a **Notification Banner**, after which an **Active Notification** may expire.
_Avoid_: Retention when referring to how long history is kept, receipt timeout when no banner was displayed

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
- A **Notification Rule** matches an **App Identity** and may additionally match **Notification Category** and **Notification Urgency**.
- An explicit drop **Notification Rule** applies even to critical notifications.
- A valid **Notification Category** is a trimmed, lower-case, dot-separated ASCII name of at most 128 bytes; malformed sender values are treated as missing metadata.
- **Notification Configuration** rejects an invalid category rule atomically without replacing the previously loaded rule slice.
- **Notification Progress** is stored as metadata and may be rendered by **Notification Banners**.
- **Notification App Icon** and **Notification Image** are distinct sender-provided visuals.
- A surface that can render both a **Notification App Icon** and a **Notification Image** should keep those visuals distinct.
- A surface that can render only one notification visual should choose the visual using freedesktop notification image priority.
- **Notification Server Request** decoding recognizes standard freedesktop visual sources for **Notification App Icon** and **Notification Image**.
- **Notification Server Request** decoding recognizes a boolean true freedesktop resident hint as **Resident Notification** behavior.
- Invalid or unresolvable sender-provided visuals do not make an otherwise meaningful notification a **Dropped Notification**.
- Invalid raw **Notification Image** data does not make an otherwise no-op notification meaningful.
- Oversized or resource-unsafe sender-provided visual payloads degrade like invalid sender-provided visuals.
- A **Body-Embedded Notification Image** may be scaled to fit a body-display surface; natural image dimensions alone do not make the image invalid when the visual can be handled safely.
- Sender-provided visual references are resolved only as local file references or icon-theme names.
- An **Accepted Notification** may retain sender-provided **Notification App Icon** references.
- An **Accepted Notification** may retain sender-provided **Notification Image** references, but not raw **Notification Image** payloads.
- Notify stores **Raw Notification Body** while displaying a conservative **Notification Body** representation.
- Existing plain **Notification Body** fields remain the visible plain-text fallback when interpreted **Notification Body Markup** is added.
- The first **Notification Body Markup** scope adds inline bold, italic, and underline emphasis while preserving a safe plain-text **Notification Body** fallback.
- Initial **Notification Body Markup** support recognizes freedesktop-relevant body tags and tolerated line-break tags, not arbitrary HTML or CSS.
- **Notification Body Segment** support preserves existing **Notification Body Markup** text emphasis and line-break semantics.
- **Notification Body Markup** tag and attribute names are interpreted case-insensitively.
- Nested **Notification Body Markup** emphasis accumulates in the shown **Notification Body**.
- Interpreted **Notification Body Markup** should not preserve parser-only text run boundaries.
- Unsupported **Notification Body Markup** does not remove its enclosed text from the **Notification Body**.
- Line-break markup contributes line breaks to the shown **Notification Body**.
- Interpreted **Notification Body Markup** may contain multiple ordered **Body-Embedded Notification Images**.
- **Notification Body** normalization trims line edges and the overall body while preserving interior text spacing.
- **Notification Body** normalization decodes XML basic entities and numeric character references.
- **Notification Body** normalization may trim whitespace-only text around **Body-Embedded Notification Images**, but valid image segments remain meaningful body content.
- Named HTML entities beyond XML basic entities are outside initial **Notification Body Markup** support.
- Malformed **Notification Body Markup** does not reject an otherwise meaningful notification.
- Malformed **Notification Body Markup** should degrade to visible text without exposing raw markup syntax when possible.
- Excessively complex **Notification Body Markup** should degrade to safe visible text rather than reject an otherwise meaningful notification.
- Hyperlink activation and body-embedded images are separate feature boundaries from initial **Notification Body Markup** support.
- Initial **Notification Body Markup** support keeps hyperlink text but does not retain hyperlink targets.
- Initial **Notification Body Markup** support may include body-embedded image alternative text, but does not retain the body-embedded image reference.
- **Body-Embedded Notification Image** alternative text contributes to the plain **Notification Body** fallback.
- A **Body-Embedded Notification Image** without alternative text does not add invented placeholder text to the plain **Notification Body** fallback.
- A body-display surface that renders a **Body-Embedded Notification Image** should not also show that image's alternative text for the same **Notification Body Segment**.
- **Notification Body Markup** does not apply to **Notification Summary**.
- **Notification Server Request** decoding maps freedesktop `Notify` method values into **Notification Intake** input.
- **Notification Intake** owns first-version **App Identity** derivation, **Notification Body** normalization, and **Notification Body Markup** interpretation.
- **Notification Intake** returns an **Accepted Notification** for sender requests that should enter **Delivery Policy** and **Notification Lifecycle**.
- **Accepted Notification** contains normalized sender/request facts, not lifecycle timestamps.
- **Notification Intake** trims **Notification Action** labels, ignores malformed **Notification Action** pairs, and keeps only the first action for each action key. An empty label remains valid only for the reserved `default` action because the XDG Notification portal's freedesktop adapter emits that pair for notification-body activation.
- **Notification Action** keys are opaque case-sensitive sender-defined values.
- Body-only notifications are valid even when **Notification Summary** is empty.
- Notifications with no **Notification Summary**, **Notification Body**, **Notification Action**, **Notification Image**, **Body-Embedded Notification Image**, or meaningful metadata are dropped as no-op notifications.
- No-op detection uses the visible **Notification Body** fallback produced from interpreted **Notification Body Markup**, not the raw sender body text.
- **Notification Intake** validates sender input and rejects no-op notifications before **Delivery Policy**.
- No-op **Notification Intake** rejection has the same sender-visible **Delivery Outcome** as a **Dropped Notification**.
- No-op **Notification Intake** rejection bypasses **Delivery Policy** but enters **Notification Lifecycle** as a dropped request so sender-visible closure is consistent.
- An **Accepted Notification** may carry the sender-provided replacement target, but interpreting replacement behavior belongs to **Notification Lifecycle**, not **Notification Intake**.
- **Do Not Disturb** silences non-critical notifications while still allowing critical **Notification Banners** by default.
- **Do Not Disturb** state persists across **Notification Daemon** restarts.
- **Do Not Disturb** takes precedence over app allow rules for non-critical notifications.
- **Do Not Disturb** is part of loaded **Delivery Policy** rule state.
- **Loaded Delivery Policy State** is assembled outside **Delivery Policy** from the **User Rule Store**, **Notification Configuration**, and persisted **Do Not Disturb** state.
- The **Delivery Policy** module may provide the builder for **Loaded Delivery Policy State**, but the **Notification Daemon** supplies already-loaded rule and **Do Not Disturb** facts.
- The **Notification Daemon** does not accept notifications until persisted **Do Not Disturb** and **User Rule Store** state has hydrated **Loaded Delivery Policy State**; failure to load persisted policy state aborts startup.
- After **Notification Daemon** startup, **Delivery Policy** uses **Loaded Delivery Policy State** without querying the **User Rule Store**, **Notification Configuration**, or persisted **Do Not Disturb** state while handling a **Notification Server Request**.
- **Loaded Delivery Policy State** changes by atomic replacement of a complete immutable snapshot. A **Delivery Policy** decision observes one complete snapshot, never a partial change from one rule source.
- Policy-source changes are serialized so concurrent **Do Not Disturb**, **User Rule Store**, and **Notification Configuration** changes cannot overwrite one another.
- A valid **Notification Configuration** reload replaces its complete rule slice in **Loaded Delivery Policy State**; partial configuration slices never become visible.
- **Do Not Disturb** and **User Rule Store** changes are persisted before replacing **Loaded Delivery Policy State**; persistence failure leaves the loaded state unchanged.
- Failures before a policy change commit point reject the operation and leave **Loaded Delivery Policy State** unchanged.
- After startup, policy reads such as **Do Not Disturb**, **User Rule Store** rules, and diagnostic policy inspection come from one **Loaded Delivery Policy State** snapshot rather than querying persisted sources.
- A committed policy change is immediately visible through policy reads and applies to subsequently handled **Notification Server Requests**.
- A **Silenced Notification** remains visible in notification history.
- Unknown **App Identities** are allowed unless a **Notification Rule** says otherwise.
- Notification delivery policy uses ordered **Notification Rules** where the first decisive matching rule determines whether a notification is shown, silenced, or dropped.
- **Delivery Policy** runs before **Notification Lifecycle** for both new and replacement notifications.
- **Delivery Policy** decides the **Delivery Outcome**; **Notification Lifecycle** acts on that outcome, including dropped notifications.
- **Delivery Policy** evaluates an immutable **Accepted Notification** and **Loaded Delivery Policy State**; it does not query storage or configuration files directly.
- The **Notification Runtime** owns the complete **Loaded Delivery Policy State** workflow: startup hydration, immutable snapshot ownership, policy reads, persisted policy changes, configuration rule-slice replacement, and policy-change serialization.
- **Delivery Policy** remains a pure module that evaluates an **Accepted Notification** against one immutable **Loaded Delivery Policy State** snapshot; it does not own loading or mutation workflows.
- First-version **Delivery Policy** rules use **App Identity** and **Notification Urgency** from the **Accepted Notification**, but the module interface accepts the full shape so future rules can use additional normalized facts.
- Initial **Notification Body Markup** styles do not affect **Delivery Policy** decisions.
- **Delivery Policy** uses the same decision path for new and replacement notifications unless a future **Notification Rule** explicitly depends on that context.
- Default allowing of unknown **App Identities** is **Delivery Policy** implementation, not a synthetic rule supplied by the **Notification Daemon**.
- **Delivery Policy** produces the diagnostic explanation used for a **Delivery Decision Log**; the **Notification Daemon** records the log without inferring policy reasons.
- **Delivery Policy** precedence and diagnostic behavior are tested through the **Delivery Policy** module interface; **Notification Daemon** tests cover rule loading, atomic snapshot replacement, and **Notification Lifecycle** wiring.
- **Delivery Decision Logs** are diagnostic debug logs, not first-version metrics.
- Normal **Delivery Decision Logs** do not include **Notification Content**.
- Normal diagnostics for **Notification Body Markup** degradation do not include **Raw Notification Body**, body text snippets, sender-provided body attribute values, or **Body-Embedded Notification Image** references.
- Normal diagnostics for invalid or unrenderable sender-provided visuals do not expose raw visual payloads or full sender-provided visual references.
- Replacement notifications revise an existing **Notification Thread** instead of creating a new normal history item.
- Replacement handling belongs to **Notification Lifecycle**, not **Notification Intake** or **Notification History**.
- Revising a **Notification Thread** replaces its retained **Accepted Notification** facts while preserving thread identity and lifecycle metadata.
- Revising a **Notification Thread** does not inherit omitted **Notification App Icon** or **Notification Image** facts from the previous revision.
- **Notification Lifecycle** owns lifecycle timestamps on **Notification Threads**.
- **Notification Lifecycle** may use an internal clock seam so lifecycle timestamp behavior is testable without moving timestamp ownership into **Notification History**.
- **Delivery Outcome** and **Rule Reference** are **Notification Thread** lifecycle metadata, not **Accepted Notification** facts.
- Notification delivery policy applies to replacement notifications; a silenced replacement updates history and closes any visible **Notification Banner** for the thread.
- A dropped replacement is handled by **Notification Lifecycle** without revising the existing **Notification Thread**.
- A dropped replacement leaves the existing **Notification Banner**, runtime image, **Notification Timeout**, and sender-visible notification ID unchanged.
- An eligible replacement notification may create a **Notification Banner** even when the previous revision did not.
- A dropped replacement notification does not update **Notification History**.
- A replacement notification that revises a **Notification Thread** cancels the prior revision's pending **Notification Timeout** before the replacement revision is considered for **Notification Banner** visibility.
- A replacement revision receives a fresh **Notification Timeout** only if it becomes visible in a **Notification Banner**.
- A new **Dropped Notification** receives a valid nonzero sender-visible ID and is immediately closed from the **Sender Notification Lifecycle** perspective with freedesktop reason `4`; its internal ephemeral ID is never a **Notification Thread** ID.
- A dropped replacement does not close the **Sender Notification Lifecycle** of the existing **Active Notification**; a later `CloseNotification` for its sender-visible ID still closes that original thread normally.
- A **Silenced Notification** may remain an **Active Notification** without a visible **Notification Banner**.
- A **Transient Notification** may produce a **Notification Banner**, **Notification Sound**, **Notification Action**, and **Notification Timeout**, but its content and lifecycle are never written to persisted **Notification History**.
- The first **Notification Center** omits **Transient Notifications** because its projection is backed by retained state; omission does not imply that the runtime notification was dropped.
- A **Notification Thread**'s initial request fixes whether it is transient or retained. Replacement `transient` hints cannot weaken a retained thread or promote a transient thread into persisted history.
- Replacing a **Transient Notification** revises the same runtime thread and sender-visible ID without creating a persisted duplicate.
- Closing, dismissing, invoking a non-resident action on, or expiring a **Transient Notification** releases its runtime thread, banner opportunity, image, and timer after publishing the normal sender-visible lifecycle effects.
- **Transient Notifications** do not survive **Notification Daemon** restart and therefore never become **Stale Notifications**.
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
- A finite **Notification Timeout** begins when the current notification revision first becomes visible in a **Notification Banner**, not when its request is accepted.
- A **Notification Timeout** that has begun continues to its original deadline if the banner is later displaced by a critical notification.
- Overflow notifications, **Silenced Notifications**, requests accepted under **Do Not Disturb**, and **Headless Mode** requests do not begin a **Notification Timeout** and deliberately remain active until user or sender action.
- A positive sender timeout uses its requested duration after banner display; a zero sender timeout never expires; a server-default timeout resolves through the configured urgency default after banner display.
- An elapsed **Notification Timeout** makes the thread an **Expired Notification** and completes its **Sender Notification Lifecycle** with freedesktop reason `1`.
- Critical **Active Notifications** do not expire automatically by default.
- First-version **Notification Timeout** configuration is global with per-urgency defaults, not per-app.
- Sender-requested **Notification Timeout** values are respected within user-configured bounds when the current revision becomes visible in a **Notification Banner**.
- **Notification Banners** appear at the top right of the focused output by default.
- **Notification Banners** are owned by the **Notification Daemon**, not by Niribar.
- First-version **Notification Image** and **Notification App Icon** rendering starts with **Notification Banners**.
- First-version **Notification Banner** visual rendering does not require full freedesktop icon-theme lookup.
- First-version **Notification Banner** file-backed visual rendering supports common static raster image formats before vector or animated image formats.
- First-version **Body-Embedded Notification Image** rendering supports common static raster image formats before vector or animated image formats.
- Notify shows at most three **Notification Banners** at once by default.
- **Notification Banner** visibility prefers critical **Active Notifications**, then newest eligible **Active Notifications**.
- Hidden **Active Notifications** are not later promoted to **Notification Banners** solely because a banner slot opens.
- **Notification Banner State** consumes **Banner Lifecycle Changes** projected from **Lifecycle Effects** and decides banner eligibility itself.
- **Notification Lifecycle** does not emit banner-specific effects.
- **Notification Banner** visibility is not derived from persisted **Notification History** flags.
- **Notification Banner** state is runtime-only and is not persisted in **Notification History**.
- Activating a **Notification Banner** invokes its **Default Notification Action** when present and otherwise dismisses the notification.
- **Default Notification Actions** are invoked by activating the notification itself, not shown as separate named action controls.
- First-version **Notification Banners** expose at most two visible named **Notification Action** controls for an **Active Notification**.
- The **Notification Center** remains the complete action surface for named **Notification Actions** on retained **Active Notifications**; **Transient Notifications** expose actions only through their runtime presentation.
- The first banner placement scope is global rather than per-output.
- If focused output cannot be determined, **Notification Banners** fall back to a configured or default output.
- First-version **Notification Banner** styling configuration is minimal.
- First-version **Notification Banner** keyboard interaction is minimal.
- **Notification Banners** do not steal keyboard focus.
- **Notification Sounds** are one-shot presentation behavior for shown notifications and are not retained in **Notification History**.
- Hiding notification body content for privacy-sensitive display states belongs to **Compositor Privacy Mode**, not Notify.
- Notify supports **Wayland Sessions** only.
- The first version supports one **Graphical User Session** per user.
- The **Notification Daemon** enforces a single active instance per **Graphical User Session**.
- The **Notification Daemon** is a coordinating owner over separate deep modules, not one monolithic module that owns every implementation detail directly.
- Separate **Notification Daemon** modules should be deep enough that their interfaces concentrate lifecycle, policy, history, or presentation rules instead of leaking those rules across callers.
- **Notification Lifecycle** is separate from **Notification History**; lifecycle rules command persistence changes rather than being owned by storage.
- **Notification Lifecycle** returns **Lifecycle Effects**; the **Notification Daemon** publishes sender-visible events and presentation updates from those effects.
- **Lifecycle Effects** are the primary **Notification Lifecycle** result; the **Notification Daemon** derives sender receipts and outward side effects from them.
- **Activation Token** generation is a sender-visible publication side effect of the **Notification Daemon**, not **Notification Lifecycle** meaning.
- A **Lifecycle Effect** may include the sender receipt for the accepted or rejected notification request.
- **Lifecycle Effects** include display-dependent **Notification Timeout** start intents and cancellation instructions; timer adapters perform waiting but do not decide timeout meaning.
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
- **Notification Server API** capabilities report only visual features that Notify can render.
- **Notification Server API** advertises **Activation Token** support only when the running **Notification Daemon** can request activation tokens from the **Wayland Session**.
- **Activation Token** support does not require every user interaction path to provide a compositor input serial.
- **Activation Token** support is enabled whenever the running **Notification Daemon** has an activation-token provider; it is not controlled by separate user configuration.
- Static notification visuals may be advertised without advertising body-embedded images or animated notification visuals.
- **Notification Server API** advertises body-embedded image support only when **Body-Embedded Notification Images** can be rendered by Notify-owned body-display surfaces.
- **Notification Server API** advertises **Notification Body Markup** only when Notify-owned body-display surfaces can preserve or render the interpreted markup semantics.
- Parser-only **Notification Body Markup** support is not enough to advertise **Notification Body Markup** through the **Notification Server API**.
- Honoring **Resident Notification** behavior does not by itself mean the **Notification Server API** advertises freedesktop notification persistence.
- The freedesktop notification compliance matrix is the source of truth for advertised, decoded-neutral, ignored, and planned **Notification Server API** behavior.
- A freedesktop capability is advertised only after sender-visible D-Bus behavior and Notify-owned presentation or lifecycle behavior are both covered by tests.
- **Notification Action** icon support treats sender action keys as static icon names only when the sender sets the freedesktop `action-icons` hint.
- Initial **Notification Body Markup** rendering is not controlled by a separate user configuration switch.
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
- The configuration module validates the full **Notification Configuration** before dispatching normalized configuration slices to runtime or presentation modules.
- Only a valid **Notification Configuration** is dispatched; partial configuration slice application is not a successful reload.
- A committed **Notification Configuration** rule slice applies to subsequently handled **Notification Server Requests** without re-evaluating **Active Notifications**.
- **Notification Daemon** and configuration errors are reported through logs and control surfaces, not self-notifications.
- The first version includes minimal **Control Commands** for daemon status, active notifications, **Do Not Disturb**, and configuration reload.
- First-version **Control Commands** are diagnostic-first and do not promise stable output formats unless explicitly documented.
- Data-returning **Control Commands** may support unstable JSON output for diagnostics and tests.
- JSON **Control Command** output that includes **Notification Body** may also include interpreted **Notification Body Markup**; human-readable **Control Command** output may remain plain text.
- JSON **Control Command** output that includes interpreted **Notification Body** content may include **Body-Embedded Notification Image** references; human-readable **Control Command** output may remain plain text.
- **Control Commands** report when the **Notification Daemon** is unavailable instead of starting it implicitly.
- The **Notification Daemon** owns notification state, lifecycle, policy, history, and rules; the **Notification Center** is an interactive view over that daemon state.
- The **Notification Daemon** owns **Activation Token** publication; **Notification Banner** presentation may supply a compositor-contextual token for the user interaction that produced the action.
- The **Notify Control API** exposes all private control surfaces over **Notification Daemon** state, including diagnostic status, active notification output, **Control Commands**, **Notification Center Projection**, and **Notification State Subscription**.
- Diagnostic **Control Commands** are adapters over the **Notify Control API** module, not a separate control path.
- The **Notify Control API** returns domain-shaped replies; command-line text, JSON, socket frames, and panel protocol messages are adapter formatting.
- **Activation Token** behavior is the same across **Notify Control API** adapters that invoke a sender-provided **Notification Action**.
- The **Notify Control API** and panel protocol do not expose **Activation Tokens**; the **Notification Daemon** publishes them through the **Notification Server API**.
- Pre-release control command formats do not require legacy compatibility.
- User interface changes to **Notification Rules** are saved in the **User Rule Store**, not by rewriting **Notification Configuration**.
- **User Rule Store** rules are keyed by **App Identity**, optional **Notification Category**, and optional **Notification Urgency**.
- **User Rule Store** matching prefers category-and-urgency, category-wide, app-and-urgency, then app-wide rules. This specificity order is deterministic regardless of persistence order.
- **Notification Configuration** preserves declared first-decisive order; authors place category-specific rules before an app-wide fallback when the category rule should win.
- **Notification Configuration** rules may define a **Config Rule ID**; unnamed config rules use generated references.
- **Notification Rules** from the **User Rule Store** take precedence over rules from **Notification Configuration**.
- Changes to **Notification Rules** affect future notifications and do not retroactively delete or reclassify **Notification History**.
- Changes to **Notification Rules** and **Do Not Disturb** do not re-evaluate **Active Notifications**. Existing **Notification Threads** keep their **Delivery Outcome** and **Notification Banner** state.
- A replacement **Notification Server Request** is evaluated under the current **Loaded Delivery Policy State**, even when it revises a **Notification Thread** created under an older policy snapshot.
- **Notification History** may store a **Rule Reference** for the rule that produced a historical **Delivery Outcome**.
- **Notification History** does not persist raw **Notification Image** payloads by default.
- **Notification History** may retain sender-provided **Notification Image** references.
- **Notification History** may retain sender-provided **Body-Embedded Notification Image** references.
- **Notification History** persists interpreted **Notification Body Markup** for new and revised **Notification Threads**.
- Legacy **Notification History** may derive interpreted **Notification Body Markup** from **Raw Notification Body** during migration, but normal history reads should not reinterpret **Raw Notification Body**.
- **Notification History** is not app-level encrypted in the first version.
- The **Notification Center** shows retained **Active Notifications** and recent **Notification History**; it omits runtime-only **Transient Notifications**.
- The **Notification Center** groups notifications by **App Identity**.
- The **Notification Center** includes an app-wide **App Permission Control** for each recent sender and category-specific controls for valid categories present in that app's retained threads.
- A category-specific control shows its explicit category rule when present and otherwise the app-wide fallback. Historical records keep their original **Delivery Outcome**.
- **Notification Center** requirements must not define core **Notification Daemon** state or lifecycle rules.
- **Notification Center Projection** is Notify-owned state and is mapped to shared panel protocol types by **Panel Provider Mode**.
- **Notification Center Projection** preserves interpreted **Notification Body Markup** in addition to the plain **Notification Body** fallback.
- **Notification Center** item activation invokes the **Default Notification Action** when present and otherwise dismisses the notification.
- **Notification Center** adapter-owned controls must not collide with sender-provided **Notification Action** keys.
- The shared panel protocol contains generic Niribar panel infrastructure, not Notify-specific notification logic.
- Shared panel protocol styled text is generic inline presentation data, not raw freedesktop notification markup.
- Shared panel protocol shape changes for generic styled text should advance the panel protocol version.
- The first Niribar-hosted **Notification Center** does not require progress rendering.
- The first Niribar-hosted **Notification Center** does not require **Notification Image** or **Notification App Icon** rendering.
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
- **Notification Actions** may be invoked from **Notification Banners** and the **Notification Center** while their **Notification Thread** is active.
- **Notification Actions** remain invokable for silenced **Active Notifications** shown in the **Notification Center**.
- **Control Commands** that activate or invoke an **Active Notification** are user-originated notification interactions for **Activation Token** purposes.
- **Activation Tokens** are emitted for any invoked sender-provided **Notification Action** when token generation succeeds.
- **Activation Tokens** identify their target application using the notification's **App Identity**.
- **Notification Banner** interactions request **Activation Tokens** with the Wayland input serial and banner surface when that click context is available, then fall back to the daemon activation-token provider.
- After a replacement, **Activation Tokens** use the current revision's **App Identity**.
- Failure to obtain an **Activation Token** does not prevent the requested **Notification Action** from being invoked.
- **Activation Token** generation is bounded; the **Notification Daemon** does not hold a **Notification Action** indefinitely while waiting for a token.
- **Activation Token** generation is attempted only after **Notification Lifecycle** accepts a **Notification Action** invocation.
- When an **Activation Token** is emitted for a non-resident **Notification Action**, sender-visible signal order is **Activation Token**, action invocation, then dismissal.
- Each **Notification Action** invocation on a **Resident Notification** gets its own **Activation Token** opportunity.
- Inactive **Notification History** is read-only except for deletion.
- Inactive **Notification History** entries do not expose invokable **Notification Action** controls.
- **Stale Notification** action labels may remain as read-only history metadata.
- Invoking a **Notification Action** dismisses the notification unless it is a **Resident Notification**.
- Non-resident **Notification Action** invocation publishes the sender-visible action event before the sender-visible dismissal event.
- **Notification Lifecycle** owns **Notification Timeout** eligibility, duration, cancellation, and expiration meaning; **Notification Banner State** reports whether the matching revision is visible, and the **Notification Daemon** starts the timer only after graphical presentation acknowledges applying that banner state.
- Inline replies are outside the first version scope.
