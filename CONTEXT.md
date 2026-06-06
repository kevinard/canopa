# Niribar

Niribar is a status bar application for the Niri Wayland compositor. It renders configurable bar surfaces on Wayland outputs and fills them with modules that show information or trigger actions.

## Language

**Bar**:
A named configuration that defines layout, output targeting, size, panel behavior, style, and module placement.
_Avoid_: Surface, window

**Bar Name**:
The user-facing identifier for a **Bar**.
_Avoid_: Surface name, output name

**Bar Surface**:
A live Wayland layer-shell surface created for a specific **Bar** on a specific **Output**.
_Avoid_: Bar, window

**Bar Surface Identity**:
The stable runtime identity of a **Bar Surface**, consisting of the **Bar Name** and the optional **Output** it targets.
_Avoid_: Output/bar tuple, window ID

**Bar Surface Registry**:
The runtime owner of live **Bar Surface** records and passive **Tooltip** metadata associated with **Bar Surfaces**. It does not own physical **Panel** or **Menu** overlay identity; that interactive overlay identity belongs to **Foreground**.
_Avoid_: Overlay lifecycle, surface manager

**Bar Surface Plan**:
A runtime description of the desired **Bar Surfaces** derived from configured **Bars** and current **Eligible Outputs** before live window reconciliation.
_Avoid_: Desired windows, surface list

**Output**:
A Wayland display target that can host zero or more **Bar Surfaces**.
_Avoid_: Screen, monitor, display

**Module**:
A configured bar content unit that can show information, trigger actions, or both.
_Avoid_: Widget, item

**Module Name**:
The user-facing identifier for a **Module** used in placement, shared state, and diagnostics.
_Avoid_: Module type, widget ID

**Module Kind**:
The category of behavior a **Module** provides, such as clock, battery, or script.
_Avoid_: Module name, instance name

**Module Descriptor**:
The static description of a **Module Kind** used to construct configured **Modules** and declare their app-owned runtime needs.
_Avoid_: Module metadata, capability flags, constructor registry

**Module Instance**:
The runtime state and behavior of a **Module** while Niribar is running.
_Avoid_: Module, widget instance

**Module Runtime**:
The app-owned runtime module that owns **Module Instances**, per-instance state, **Module Actions**, origin metadata, interaction profiles, rendering, **Tooltip** values, **Module Data Source** mapping, **Module Detail Session** routing, and dynamic-child storage behavior. Its external interface may return Iced elements because Iced rendering is part of Niribar's current runtime interface.
_Avoid_: Module storage, widget runtime

**Dynamic Module**:
A **Module** that produces runtime child **Module Instances** from external state.
_Avoid_: Generated module, synthetic module

**Dynamic Module Child Key**:
The stable runtime identity used by the **Dynamic Module Lifecycle** to match one desired child **Module Instance** with an existing child **Module Instance** for the same **Dynamic Module**.
_Avoid_: Generated ID, child widget key

**Dynamic Module Family**:
A specific **Dynamic Module** behavior family with a parent **Module Kind**, child **Module Kind**, **Dynamic Module Child Key** rule, child construction rule, and child update policy.
Examples: tray icons produced by the tray **Dynamic Module**, workspace items produced by the workspaces **Dynamic Module**.
_Avoid_: Generic dynamic module helper, generated module type

**Dynamic Module Family Input**:
A normalized, family-specific input derived from parent **Module** configuration and inherited runtime metadata before the **Dynamic Module Lifecycle** constructs or updates child **Module Instances**. It contains only facts needed by that **Dynamic Module Family**, not the full typed configuration shape.
_Avoid_: Raw module config, family options, constructor params

**Dynamic Module Child Fact**:
A source-shaped runtime fact used by the **Dynamic Module Lifecycle** to construct, update, rebuild, or remove one child **Module Instance** for a **Dynamic Module Family**.
Examples: an active SNI tray item, a compositor-reported **Workspace**.
_Avoid_: Child config, generated child state

**Dynamic Module Reconciliation Plan**:
A pure runtime description of changes the **Dynamic Module Lifecycle** wants applied after reconciling **Dynamic Module Child Facts**, including child **Module Instance** insertion, removal, in-place update, inherited runtime metadata, and stale **Keyboard Focus** or hover cleanup targets. It does not cache expanded **Bar** or **Panel** layouts; layout expansion is a separate pure **Dynamic Module Lifecycle** interface over the current runtime snapshot.
_Avoid_: App mutation, callback result, generated diff

**Dynamic Module Runtime Snapshot**:
A narrow, read-only snapshot of the app-owned runtime facts the **Dynamic Module Lifecycle** needs to reconcile or expand **Dynamic Modules**, including ordered **Module Instance** identity, **Module Kind**, **Zone**, parent **Module Name**, current order, inherited **Module Actions**, interaction profile, and normalized **Dynamic Module Family Inputs** needed for child construction. Existing child **Module Instances** cross the snapshot as small child facts rather than full implementations; in-place update families use a narrow update hook instead of exposing all child storage.
_Avoid_: Module Runtime reference, storage view, full runtime state

**Dynamic Module Lifecycle**:
The app-owned runtime lifecycle that reconciles **Dynamic Module** child source facts into child **Module Instances**, owns child construction, clears stale child targets, preserves inherited actions and interactions, and expands child placement in **Bar** and **Panel** module layouts.
_Avoid_: Dynamic module cleanup, generated module registry

**Module Data Source**:
An app-owned runtime source of changing domain state used by one or more **Module Instances**.
_Avoid_: Module subscription, backend, provider

**Module Data Source Identity**:
The stable runtime identity of a **Module Data Source**. Multiple **Module Instances** may depend on the same **Module Data Source Identity** when they share one source of truth.
_Avoid_: Module kind, subscription key, source id

**Module Data Source State**:
The source-shaped runtime state emitted by a **Module Data Source** before it is mapped into per-**Module Instance** render state.
_Avoid_: Module state, widget state

**Module Data Source Runtime**:
The app-owned runtime module that owns **Module Data Source** lifecycle, active source requirements, subscriptions, first emission, recovery, and **Unavailable Module** transitions before handing source-shaped state to **Module Runtime**.
_Avoid_: Module Runtime, DBus adapter

**Zone**:
A logical placement group within a **Bar**, named **Start**, **Center**, or **End**.
_Avoid_: Top, bottom, left, right

**Edge**:
The side of an **Output** where a **Bar Surface** is anchored: top, bottom, left, or right.
_Avoid_: Orientation, side

**Edge Span**:
The length a **Bar Surface** occupies along its **Edge** after accounting for other **Bar Surfaces** on the same **Output**.
_Avoid_: Full width, full height

**Layer**:
The compositor stacking layer used by a **Bar Surface**.
Default: top.
Allowed values: background, bottom, top, overlay.
_Avoid_: Popup layer, z-index

**Orientation**:
The layout direction of a **Bar**, derived from its **Edge**.
_Avoid_: Edge

**Output Target**:
An exact include-list or exclude-list expression that selects which **Eligible Outputs** a **Bar** should render on.
_Avoid_: Output override, monitor filter

**Eligible Output**:
A currently active Wayland **Output** that is not excluded by configuration.
_Avoid_: Niri output, connected output

**Excluded Output**:
An **Output** that configuration prevents Niribar from rendering on.
_Avoid_: Disabled bar, ignored monitor

**Unavailable Module**:
A **Module** whose data source cannot currently provide useful data.
_Avoid_: Failed module, hidden module

**Empty Module**:
A **Module** whose data source is available but has no current content to show.
_Avoid_: Unavailable module, failed module

**Configuration Error**:
An invalid user configuration that prevents Niribar from constructing the requested model.
_Avoid_: Unavailable module, runtime failure

**Module Action**:
An intentional user-triggered effect caused by interacting with a **Module**.
_Avoid_: Event, subscription, refresh, panel action when the action is not module-scoped

**Module Interaction**:
A pointer or keyboard input delivered to a **Module Instance** before it resolves into a **Module Action**, **Menu**, or module-owned effect.
_Avoid_: Module action, app event, key binding

**Tooltip**:
Supplemental information shown for a **Module** on a **Bar Surface**.
_Avoid_: Popup action, module display

**Menu**:
An interactive popup exposed by a **Module** or child **Module Instance**.
_Avoid_: Tooltip, action, launcher

**SNI Menu**:
A **Menu** exposed by a StatusNotifierItem through DBusMenu and rendered by Niribar as a popup.
_Avoid_: Panel Menu, launcher, configured menu

**Menu Search**:
The user-facing matching behavior that filters or ranks selectable **Menu** or **Panel Menu** entries from typed input.
_Avoid_: Panel Menu Search when the behavior is not panel-only, filter when referring to ranked matching behavior

**Custom Menu Value**:
A user-submitted typed value returned by a selectable menu instead of an existing entry.
_Avoid_: Dmenu entry when the value was typed by the user, menu item when no configured item exists

**Selectable Menu Session**:
The live runtime interaction for one selectable **Menu** or **Panel Menu**, including its entry source, active menu level, optional **Menu Search** text, focused entry, hover state, optional **Custom Menu Value** affordance, and selected-entry completion. The active **Menu** session is owned by **Foreground**; **Panel Menu** sessions are owned by **Foreground** through the active **Panel Runtime** while rendered inside a **Panel**. It does not own physical **Menu** or **Panel** window lifecycle.
_Avoid_: Panel Menu Session when the behavior applies beyond panels, popup state when referring to selection behavior

**Foreground**:
The currently active interactive overlay state, either no overlay, a **Menu**, or a **Panel**. **Foreground** owns physical **Panel** and **Menu** overlay window identity, the active **Menu**'s **Selectable Menu Session**, the active **Panel Runtime**, replacement, stale close handling, interactive overlay role classification, and all app-level **Panel** interaction routes. It is a pure state machine module with an intent/effect interface; callers provide required pure context on intents, and app-owned side effects remain in the app adapter. **Tooltip** is not **Foreground** because it is passive, non-interactive, and owned as a **Bar Surface** affordance.
_Avoid_: overlay registry when referring to interactive state, popup when referring to **Panel** or **Menu** interaction

**Panel**:
A transient overlay surface opened from a **Bar Surface**. A **Panel** is anchored to its originating **Bar Surface** or **Module** and can render module detail content, a **Bar**'s configured panel module layout, or named **Panel Menu** content.
_Avoid_: Popup when referring to Niribar's generic panel

**Panel Sizing**:
The content-aware policy that determines how much screen space a **Panel** occupies while respecting its **Panel Origin** and content kind. **Panel Sizing** keeps **Module Detail Content** compact with content-preferred sizes, uses **Edge Span** fill sizing for **Panel Menu** content and a **Bar**'s configured panel module layout, uses preferred sizing for the non-**Edge Span** axis of a configured panel module layout, treats explicit configured dimensions as overrides, applies configured maximum dimensions as caps, and falls back to default dimensions when no content-preferred size exists.
_Avoid_: Popup size, fixed panel size

**Panel Runtime**:
The internal **Foreground**-owned module for the live content and interaction state of the currently active **Panel**, including its **Selectable Menu Session** when a **Panel Menu** is present, pending search focus, menu scroll position, **Module Detail Session** state, and any active **Dmenu Selection Request** responder. It does not expose an app-level interface and does not own physical **Panel** window effects.
_Avoid_: Panel Lifecycle, overlay lifecycle, physical Panel window identity

**Panel Lifecycle**:
The **Foreground**-owned logical lifecycle of the currently active **Panel** from accepted open request through opening, visible, replacement, and close.
_Avoid_: Panel popup lifecycle, overlay lifecycle when referring to logical **Panel** ownership

**Panel Attachment**:
The internal **Foreground** association between the active **Panel** and the physical overlay window that renders it. It may be represented by an internal identity token, but it is not a separate app-level seam and is not owned by **Panel Runtime** or the **Bar Surface Registry**.
_Avoid_: app-level panel seam, Bar Surface Registry panel metadata, Panel Runtime window identity

**Panel Origin**:
The **Bar Surface** or **Module** that triggered the current **Panel**.
_Avoid_: Active module when referring specifically to panel source

**Module Detail Content**:
Module-owned content rendered when a **Panel** is opened from a specific **Module**.
_Avoid_: Panel menu, popup, separate module when the content belongs to the originating module

**Module Detail Session**:
The transient module-owned state and interaction lifecycle for **Module Detail Content** while its **Panel** is open. It is created fresh when the **Panel** opens and discarded when that **Panel** closes or is replaced.
_Avoid_: Panel runtime state, global module state, persisted detail view

**Panel Action**:
A typed configured operation triggered by pointer input, focused keyboard input, **Panel Menu** item activation, or IPC.
Examples: `open-panel`, `toggle-panel`, `close-panel`, `open-menu`, `toggle-menu`, `command`.
_Avoid_: Shell action when the action is not a command

**Configured Command**:
A user-declared command value from configuration, either shell text or an explicit argv program and argument list. It can power script content, command-backed **Module Actions**, and command-backed **Panel Actions**.
_Avoid_: Shell command when the value may be argv; action when referring only to the command value

**Command Policy**:
The configuration-owned policy for **Configured Commands**, including whether shell-form commands are accepted and how long command-backed **Module Actions** and **Panel Actions** may run before Niribar kills and reaps the process. It does not define **Dmenu Selection Mode** selection waiting or **Installed Desktop Application** launch behavior.
_Avoid_: Script timeout, desktop launch policy, security mode

**Panel Menu**:
An app-owned, named launcher/menu list rendered inside a **Panel**.
_Avoid_: Menu when referring to SNI or other module-owned context menus

**Panel Menu Session**:
The **Selectable Menu Session** for one opened **Panel Menu**.
_Avoid_: Panel state when referring only to menu selection behavior

**Panel Menu Entry Icon**:
An optional XDG icon-theme name attached to a **Panel Menu** entry.
_Avoid_: Installed app icon when referring to the general menu entry capability

**Panel Menu Source**:
The configured origin of **Panel Menu** entries, either explicit user configuration or a dynamic runtime source such as installed desktop applications.
_Avoid_: Generated config, launcher backend

**Panel Menu Prompt**:
Optional text that gives user-facing context for a **Panel Menu** search or selection.
_Avoid_: Menu item, title when the text is not just a heading

**Installed Desktop Application**:
A user-launchable GUI application discovered from the desktop environment's application metadata.
_Avoid_: Desktop file, command, package

**Desktop Application ID**:
The stable identifier Niribar uses to distinguish one **Installed Desktop Application** from another.
_Avoid_: Application name, label

**Keyboard Focus**:
The current **Module** or child **Module Instance** target selected for keyboard interaction on a **Bar Surface**.
_Avoid_: Global focus, compositor focus

**Compositor Integration**:
An optional connection to a Wayland compositor that enriches **Modules** with compositor-specific information or **Module Actions**.
_Avoid_: Runtime requirement, platform

**Niri Integration**:
The **Compositor Integration** that connects Niribar to Niri IPC.
_Avoid_: Niri runtime, Wayland output source

**Workspace**:
A compositor-provided work area shown by workspace-capable **Modules**.
_Avoid_: Niri workspace, desktop

**Focused Window**:
The compositor-reported window that currently has global input focus.
_Avoid_: Active window

**Active Window**:
The compositor-reported window active on a specific **Output**.
_Avoid_: Focused window

**Bar Surface Interaction**:
A user interaction with a **Bar Surface** that is not targeted at a **Module**.
_Avoid_: Module action

**Global Panel Toggle**:
An IPC-triggered request to toggle the default **Panel** without direct **Bar Surface** interaction context.
_Avoid_: Toggle all, expansion toggle

**Dmenu Selection Mode**:
A one-shot Niribar invocation that reads candidate entries from standard input, shows a searchable selector, writes the selected entry to standard output, and exits.
_Avoid_: Dmenu compatibility mode, launcher mode, panel menu mode

**Dmenu Selection Request**:
A single request sent from a standalone **Dmenu Selection Mode** CLI process to the running Niribar instance for interactive selection.
_Avoid_: Panel action, IPC command, configured menu

**Dmenu Panel Menu Source**:
A request-owned **Panel Menu Source** whose entries come from a **Dmenu Selection Request**.
_Avoid_: Installed applications source, configured source

**Dmenu Entry Stream**:
A line-oriented standard-input format that supplies one candidate entry per record to **Dmenu Selection Mode**.
_Avoid_: Menu config, bulk menu payload

**Rich Dmenu Entry**:
A structured **Dmenu Entry Stream** record that can provide separate selection value and panel-menu metadata such as label, icon, and keywords.
_Avoid_: Configured menu item, installed application

**Global Defaults**:
Configuration values used by **Bars** or **Modules** when they do not provide their own values.
_Avoid_: Base bar, inherited config

**Implicit Bar**:
The default **Bar** Niribar uses when configuration declares no **Bars**.
_Avoid_: Default config, fallback surface

## Relationships

- A **Bar** may produce zero, one, or many **Bar Surfaces**.
- **Bar Names** are globally unique, exact, and case-sensitive.
- Duplicate **Bar Names** are **Configuration Errors**.
- **Bar Names** and **Module Names** live in separate namespaces.
- An **Implicit Bar** exists only when configuration declares no **Bars**.
- The **Implicit Bar** uses `main` as its diagnostic **Bar Name**, but `main` is not reserved for declared **Bars**.
- An **Implicit Bar** uses **Global Defaults** and applies to all **Eligible Outputs**.
- For **Bar Surface Plan** purposes, any **Bar** with no explicit **Output Target** applies to all screens when **Eligible Outputs** are unknown, whether it is implicit or declared alongside other **Bars**.
- The **Bar Name** is not the condition that enables all-screens planning.
- A **Bar Surface** belongs to exactly one **Bar** and exactly one **Output**.
- A **Bar Surface Identity** identifies one desired or live **Bar Surface** independently of its layer-shell window ID.
- A **Bar Surface Plan** contains desired **Bar Surface Identities** and enough **Bar** lookup information to create missing **Bar Surfaces**.
- A **Bar Surface Plan** is computed before the **Bar Surface Registry** reconciles desired **Bar Surfaces** with live **Bar Surfaces**.
- A **Bar Surface Plan** does not own live windows, overlay metadata, or **Panel Runtime** interaction state.
- In AllScreens mode, a **Bar Surface Identity** has no specific **Output** and identifies an all-screens **Bar** being planned.
- The **Bar Surface Registry** is the source of truth for which live windows are **Bar Surfaces** and for passive **Tooltip** metadata associated with **Bar Surfaces**.
- The **Bar Surface Registry** does not own **Panel Runtime** interaction state, **Foreground** state, or physical **Panel** and **Menu** overlay identity.
- A **Bar Surface** uses the available **Edge Span** by default.
- A **Bar Surface** uses the **Layer** configured by its **Bar**.
- A **Bar Surface** remains present even when all of its **Modules** are empty or unavailable.
- An **Output** may host zero, one, or many **Bar Surfaces**.
- **Output** names are matched exactly and case-sensitively.
- Niribar creates **Bar Surfaces** only for **Eligible Outputs**.
- An **Excluded Output** is not an **Eligible Output**.
- Global **Excluded Outputs** are named with plain exact **Output** names, not negative **Output Target** syntax.
- An **Excluded Output** name that does not match a currently connected **Output** is valid.
- Global **Excluded Outputs** take precedence over **Output Targets**.
- Duplicate global **Excluded Output** names are **Configuration Errors**.
- Multiple **Bar Surfaces** may share the same **Output** and **Edge**.
- Niribar does not automatically resolve overlap between **Bar Surfaces** that share the same **Output** and **Edge**.
- Niribar creates horizontal **Bar Surfaces** before vertical **Bar Surfaces** so the compositor can allocate remaining **Edge Span** to vertical surfaces.
- A **Bar** with no **Output Target** applies to all eligible **Outputs**.
- `all` is the default **Output Target** value for a **Bar**.
- For runtime planning, omitted and explicit all-screens **Output Targets** are equivalent.
- A positive **Output Target** list matches any listed exact **Output** name.
- A negative **Output Target** list applies to all **Eligible Outputs** except the listed exact **Output** names.
- A single **Output Target** cannot mix positive and negative entries.
- Duplicate entries in one **Output Target** are **Configuration Errors**.
- An **Output Target** that matches no currently **Eligible Outputs** is valid.
- **Output Targets** are additive: if multiple **Bars** match one **Output**, Niribar creates multiple **Bar Surfaces** for that **Output**.
- A **Bar** may override **Global Defaults**.
- Output-specific bar behavior is modeled by defining another named **Bar** with an **Output Target**, not by mutating a **Bar** per output.
- Per-output **Bar** overrides are outside Niribar's intended configuration model.
- A **Bar** contains zero or more **Modules**.
- A **Bar** with no **Modules** is valid and may still produce **Bar Surfaces**.
- A **Module** belongs to exactly one **Zone** within a **Bar**.
- A **Module** may be placed in multiple **Bars**.
- Placing the same **Module** more than once within one **Bar** is a **Configuration Error**.
- A **Module** is configured globally by name; a **Bar** places **Modules** but does not redefine their internals.
- A **Module** reused across **Bars** shares runtime state by name when its data source is global.
- Separately named **Modules** of the same kind do not share runtime state.
- A synthesized built-in **Module** uses its **Module Kind** as its **Module Name**.
- **Module Names** are globally unique.
- Duplicate **Module Names** are **Configuration Errors**.
- All built-in **Module Kinds** reserve their default **Module Names**, whether or not they can be synthesized.
- A declared **Module** may use a reserved built-in **Module Name** only when its **Module Kind** matches that reserved name.
- A declared **Module** may override **Global Defaults** for its matching built-in **Module Kind**.
- A declared **Module** may not change the **Module Kind** associated with a reserved built-in **Module Name**.
- **Module Name** matching is exact and case-sensitive.
- A placement name resolves first to a declared **Module Name**, then to a built-in **Module Kind** with meaningful, safe **Global Defaults**.
- A placement name that resolves to neither a declared **Module Name** nor a synthesizable built-in **Module Kind** is a **Configuration Error**.
- A **Module** may render differently on different **Bar Surfaces** when its information is output-sensitive.
- A **Module** may expose zero or more **Module Actions**.
- A failed **Module Action** does not change the normal **Module** display by default.
- Output-sensitive **Module Actions** use the originating **Bar Surface** context.
- A **Module Action** may run a **Panel Action** when configured.
- A **Module** may expose a **Tooltip**.
- A **Module** may expose a **Menu**.
- A **Dynamic Module** owns the placement and configuration for its runtime child **Module Instances**.
- The **Dynamic Module Lifecycle** is explicit about supported **Dynamic Module Families** rather than acting as a generic callback helper.
- Tray and **Niri Integration** pass **Dynamic Module Child Facts** to the **Dynamic Module Lifecycle**; they do not construct child **Module Instances**.
- The **Dynamic Module Lifecycle** owns placeholder lookup, **Dynamic Module Child Key** matching, parent **Module** configuration lookup, inherited **Module Actions**, child construction, child reconciliation, stale focus and hover cleanup, inherited actions and interactions, and layout expansion for **Dynamic Modules**.
- The **Dynamic Module Lifecycle** returns a **Dynamic Module Reconciliation Plan** rather than directly mutating **Module Runtime**.
- The **Dynamic Module Lifecycle** consumes a **Dynamic Module Runtime Snapshot**, not a raw **Module Runtime** reference.
- The **Dynamic Module Lifecycle** lives in the app runtime, with **Module Runtime** acting as the app-level interface and storage owner for ordered **Module Instances**, actions, origins, and interaction profiles.
- App callers should cross the **Module Runtime** interface for **Module Instance** rendering, **Tooltip** values, **Module Data Source** mapping, **Module Interaction** dispatch, **Module Detail Session** routing, and dynamic child storage behavior instead of calling optional hooks on concrete **Module** implementations directly.
- The **Module Runtime** interface uses query methods for read-only behavior such as **Module Instance** rendering, **Tooltip** values, and layout expansion, and uses intent/effect handling for mutating behavior such as **Module Interaction**, **Module Detail Session** updates, and dynamic child reconciliation.
- **Module Runtime** applies **Dynamic Module Reconciliation Plans** to durable child **Module Instance** storage, including child removal, in-place update, insertion, inherited **Module Actions**, origins, interaction profiles, and metadata pruning.
- App callers apply only the effects returned by **Module Runtime** from dynamic child reconciliation, such as stale **Keyboard Focus** or hover cleanup.
- Deepening **Module Runtime** should first add Module Runtime interface methods that delegate to the current concrete **Module** implementation hooks, then migrate App callers to those methods, and only then shrink or split the internal concrete **Module** implementation traits.
- Splitting the internal concrete **Module** implementation interface is gated until no app caller bypasses **Module Runtime** by reading concrete **Module** hooks, iterating raw **Module Instance** storage, indexing raw storage, or directly reading or mutating per-**Module Instance** state.
- After the internal concrete **Module** implementation interface split gate is satisfied, the first split should isolate **Module Instance** rendering behavior and **Tooltip** value projection before source mapping, interaction, detail-session, or dynamic-child behavior.
- The first internal rendering split should remain Iced-element-shaped; it should not introduce an intermediate render model unless a later requirement makes that a real source of leverage.
- The first internal rendering split includes **Module Detail Content** rendering, including rendering with an existing **Module Detail Session**; creating or updating a **Module Detail Session** remains separate detail-session behavior for a later split.
- The first internal rendering split should centralize fallback selection in the internal render adapter or **Module Runtime**, not as broad default methods on another concrete **Module** implementation trait.
- The first internal rendering split should use separate render forms for distinct **Module Instance** presentations rather than one mode-enum render method; collapsed and panel rendering are the required forms.
- The **Dynamic Module Lifecycle** is stateless; durable child **Module Instance** state remains in **Module Runtime**, and stale visible-target cleanup remains applied through **Bar Surface Registry** state.
- Child-origin mapping for **Dynamic Modules** is internal to the **Dynamic Module Lifecycle**; other app modules should ask the lifecycle for the configured **Module Name** instead of inspecting child **Module Instances** to infer the parent **Dynamic Module**.
- Callback-shaped child replacement methods are transitional implementation details only; future **Dynamic Module Families** should cross the keyed **Dynamic Module Lifecycle** interface.
- The applied deepening path for **Dynamic Module Lifecycle** routes layout expansion and configured **Module Name** lookup through a **Dynamic Module Runtime Snapshot**, and routes child reconciliation through **Dynamic Module Reconciliation Plans**.
- The **Dynamic Module Lifecycle** supports per-family child update policy: some **Dynamic Modules** update matching child **Module Instances** in place, while others rebuild matching child **Module Instances** from current child source facts.
- The **Dynamic Module Lifecycle** owns per-family child ordering policy. Workspace child **Module Instances** are ordered by compositor workspace index; tray child **Module Instances** are ordered deterministically by SNI runtime identity.
- **Dynamic Module Lifecycle** tests should cover keyed reconciliation, in-place child update, matching-child rebuild, stale **Keyboard Focus** and hover cleanup, inherited **Module Actions** and interaction profiles, and identical expansion in **Bar** and **Panel** module layouts.
- Child **Module Instances** produced by a **Dynamic Module** are individually navigable and actionable when they represent distinct user targets.
- A **Module Instance** may depend on a **Module Data Source**.
- Multiple **Module Instances** may share one **Module Data Source Identity**.
- A **Module Data Source** emits **Module Data Source State**, not render-specific **ModuleState**.
- **Module Data Source Runtime** maps active **Module Runtime** source requirements into subscriptions and owns first emission, recovery, and **Unavailable Module** transitions for **Module Data Sources**.
- **Module Runtime** maps **Module Data Source State** into per-**Module Instance** render state.
- Pending first emission is internal **Module Data Source Runtime** state, not a public render state for **Module Implementations**.
- When a **Module Data Source** recovers and emits valid **Module Data Source State**, **Module Runtime** automatically remaps all dependent **Module Instances**.
- A **Module Implementation** maps available **Module Data Source State** into either visible render state or an explicit **Empty Module** outcome.
- An **Empty Module** outcome is module-owned; runtime lifecycle does not infer it from blank rendered content.
- The visible **Module** or child **Module Instance** target owns the **Tooltip** shown for that target.
- A **Tooltip** closes when its originating visible target disappears.
- A **Tooltip** closes when a **Panel** opens.
- A **Tooltip** is not **Foreground**.
- A **Bar Surface** shows at most one **Tooltip** at a time.
- Opening a **Menu** closes open **Tooltips** on the same **Bar Surface**.
- Niribar shows at most one **Menu** at a time.
- A **Menu** closes when its originating visible target disappears.
- **Foreground** owns the active **Menu**'s **Selectable Menu Session**.
- A **Menu** session is keyed by current **Foreground**, not by a map of physical window IDs.
- Opening a **Panel** closes any open **Menu**.
- Opening a **Menu** closes any open **Panel**.
- Niribar has at most one **Foreground** at a time.
- Opening a **Panel** or **Menu** replaces the current **Foreground**.
- Every request that opens, toggles, or closes a **Panel** or **Menu** goes through **Foreground**.
- App-level **Panel** open, toggle, close, search, focus, activation, and module-detail interaction intents cross the **Foreground** interface; callers do not talk to **Panel Runtime** directly.
- **Foreground** owns stale close handling for physical **Panel** and **Menu** overlay windows.
- **Foreground** is the source of truth for interactive overlay role classification.
- **Foreground** intents carry the pure context required to prepare or update **Panel Runtime** state, such as **Panel Menu** source configuration and **Module Detail Session** values.
- **Foreground** communicates side-effect work through effects; it does not directly create windows, call DBus, launch processes, send IPC responses, load installed applications, or mutate passive **Tooltip** metadata.
- When a **Panel** interaction needs impure work or external data, **Foreground** emits an effect and resumes from an explicit completion intent.
- The app adapter translates **Foreground** effects into layer-shell window tasks, DBus calls, process launches, IPC responses, installed application loading, and **Tooltip** close work.
- **Foreground** tests are the primary test surface for interactive overlay replacement, stale close handling, and current close handling.
- App tests for **Foreground** behavior should focus on adapter effect application order, not duplicate **Foreground** state-machine decisions.
- **Keyboard Focus** enters a **Menu** while it is open and returns to the originating target when the **Menu** closes, if that target still exists.
- A **SNI Menu** is a **Menu**.
- **SNI Menus** do not show **Menu Search** by default.
- A user may configure **Menu Search** for **SNI Menus**.
- A **SNI Menu** with configured **Menu Search** renders a visible search input while the **Menu** is open.
- When a **SNI Menu** layout updates while open, Niribar resets that **Menu** to its top level with empty **Menu Search** text and initial focus.
- **Menus** and **Tooltips** do not inherit the **Layer** of their **Bar Surface**.
- Niribar shows at most one **Panel** at a time.
- **Foreground** owns **Panel Lifecycle** open, toggle, replacement, and close decisions.
- **Panel Runtime** is an internal **Foreground**-owned module; **Foreground** prepares, clears, and routes **Panel** interactions to it directly.
- A **Panel** becomes logically active when **Foreground** accepts the open request and prepares **Panel Runtime** content state, before its overlay window is created.
- Replacing a **Panel** makes the old **Panel** logically gone immediately, even while its old overlay window is still closing.
- An external **Panel** window close clears **Panel Runtime** state only through **Foreground** and only when **Foreground** identifies that window as the current **Panel**.
- The **Panel Runtime** owns the current **Panel** content state used for rendering and interaction, but that state is exposed to callers through **Foreground** read interfaces.
- Callers render a **Panel** from a **Foreground** **Panel** render snapshot, not by asking **Panel Runtime** for raw menu render state.
- Callers update **Module Detail Session** state through the active **Panel** session exposed by **Foreground**; the active **Panel** check and session lookup stay local to **Foreground**.
- **Panel** focus movement and keeping the focused **Panel Menu** entry in view are one **Foreground** interaction; callers provide scroll measurement context and apply the resulting scroll effect.
- **Panel Action** requests produced from **Panel Runtime** or **Module Detail Session** state are resolved through **Foreground**, which attaches the active **Panel Origin** before the app adapter dispatches the action.
- **Panel Runtime** may request closing the active **Panel** after a Panel-owned interaction completes; **Foreground** interprets that request and still owns the **Panel Lifecycle** close decision and physical **Panel** window effects.
- **Panel Runtime** does not emit app-level physical **Panel** window open or close effects.
- **Foreground** owns physical **Panel** overlay windows for classification and stale close handling.
- A **Panel Attachment** is internal to **Foreground** and may be absent while a **Panel** is opening.
- Replacing a **Panel** replaces the internal **Panel Attachment** inside **Foreground**.
- A **Panel** belongs to exactly one originating **Bar Surface**.
- A **Panel** has exactly one **Panel Origin**.
- A **Panel Origin** may be the **Bar Surface** itself or a **Module** on that **Bar Surface**.
- A **Panel** does not change the size, **Edge Span**, or normal module rendering of its originating **Bar Surface**.
- A **Panel** renders as an overlay and does not reserve exclusive screen space.
- A **Panel** opens beside the **Edge** of its originating **Bar Surface**.
- A module-originated **Panel** aligns to the originating **Module** when its bounds are available and falls back to the **Bar Surface** otherwise.
- The **Panel Origin** remains visually marked while the **Panel** is open.
- A **Panel** closes when its originating **Bar Surface** disappears.
- A **Panel** closes when Escape or an explicit close action closes it, or when its originating **Bar Surface** disappears.
- A **Panel** may remain open when focus moves from the originating **Bar Surface** into the **Panel**.
- Escape closes a **Panel** unless the active **Panel** content has a more specific first-Escape behavior.
- Omitting an exact configured **Panel** dimension lets **Panel Sizing** choose that dimension from content, **Edge Span** fill behavior, or fallback defaults.
- A **Panel** with an explicit configured dimension larger than the corresponding configured maximum dimension is a **Configuration Error**.
- A **Module** may provide default **Module Detail Content** for its own module-originated **Panel**.
- A **Module** that provides **Module Detail Content** may provide a content-preferred size for that detail content.
- A **Module Detail Session** belongs to the **Module Detail Content** of one open **Panel**.
- A **Module Detail Session** is module-owned; generic **Panel Runtime** does not interpret module-specific detail events or detail state.
- A **Module Detail Session** resets each time module-originated **Module Detail Content** opens.
- A **Module Detail Session** may request app-owned effects for shared app infrastructure rather than reimplementing those effects inside the **Module**.
- Clock **Module Detail Content** shows the current local month by default and may let the user browse other months while that **Panel** remains open.
- Clock month browsing resets to the current local month each time the clock **Module Detail Content** opens.
- Clock month browsing is available through pointer controls and keyboard controls while the clock **Module Detail Content** is focused.
- Clock month browsing provides a way to return directly to the current local month.
- Clock **Module Detail Content** marks today when today is visible and does not select individual days.
- Clock **Module Detail Content** displays calendar weeks starting on Monday.
- Clock **Module Detail Content** leaves calendar cells outside the displayed month blank.
- Clock calendar detail behavior is not user-configurable unless a later requirement introduces a real user choice.
- A **Bar** may configure a default **Panel** module layout using **Start**, **Center**, and **End** groups.
- A **Bar**'s default **Panel** layout is independent from the **Bar**'s normal module layout.
- A **Bar**'s default **Panel** layout renders as ordered sections, not as stretched bar zones.
- A **Panel** module layout may reference **Modules** that are not present in the **Bar**'s normal module layout.
- Placing the same **Module** more than once within one **Panel** module layout is a **Configuration Error**.
- **Dynamic Modules** expand in **Panel** module layouts the same way they expand in **Bar** module layouts.
- **Panel Actions** are typed operations, not shell commands by default.
- A **Panel Action** trigger may be a pointer trigger such as `click` or `right-click`, or a focused keybind such as `Ctrl+E`.
- **Module** keybind **Panel Actions** fire only while that **Module** is the **Keyboard Focus** target.
- **Bar** keybind **Panel Actions** fire while that **Bar Surface** or its **Panel** has keyboard focus.
- If both a focused **Module** and its **Bar** define the same keybind, the **Module** binding takes precedence.
- A **Bar Surface Interaction** may trigger a **Panel Action** when configured by the **Bar**.
- A **Bar Surface Interaction** on empty bar background is distinct from interacting with a **Module**.
- A **Panel Menu** is global named configuration and can be opened from any **Panel Action**.
- A **Panel Menu** has exactly one **Panel Menu Source**.
- A **Panel Menu** controls whether **Panel Menu Entry Icons** are rendered for its entries.
- A **Panel Menu** does not render **Panel Menu Entry Icons** unless icon rendering is explicitly enabled.
- A **Panel Menu** applies icon rendering consistently across its menu levels and search results.
- A **Panel Menu** declares icon rendering as menu metadata, not as a menu entry or source declaration.
- A **Panel Menu Prompt** is menu metadata, not a selectable menu entry.
- A **Panel Menu Prompt** may appear as a search input placeholder, panel header, or both.
- **Panel Menu Prompt** rendering is a general **Panel Menu** capability, even when the first public caller is **Dmenu Selection Mode**.
- A **Panel Menu** that renders **Panel Menu Entry Icons** preserves row label alignment when an entry has no icon or its icon cannot be resolved.
- A **Panel Menu Source** may be declared by explicit `item` configuration or by a `source` declaration.
- A dynamic **Panel Menu Source** declaration starts with a source kind and may gain source-specific options later.
- A **Panel Menu** cannot mix explicit `item` configuration with a dynamic `source` declaration.
- A **Panel Menu** item runs exactly one **Panel Action**.
- A **Panel Menu** item may declare at most one **Panel Menu Entry Icon**.
- A static **Panel Menu Entry Icon** with an empty or path-like name is a **Configuration Error**.
- A dynamic **Panel Menu Source** may supply **Panel Menu Entry Icons** from its own entry metadata.
- A dynamic **Panel Menu Source** supplies current **Panel Menu** entries at runtime without rewriting user configuration.
- A dynamic **Panel Menu Source** with no current entries is a valid empty **Panel Menu**, not a **Configuration Error**.
- An installed-applications **Panel Menu Source** includes **Installed Desktop Applications**, not hidden, terminal-only, or system desktop entries.
- Desktop-environment-specific application metadata is honored when deciding whether an **Installed Desktop Application** belongs in the current environment.
- Activating an **Installed Desktop Application** from a **Panel Menu** launches the application and closes the **Panel**.
- Launching an **Installed Desktop Application** from a **Panel Menu** is source-owned behavior, not a separate public **Panel Action** type.
- Launching **Installed Desktop Applications** follows Niribar's centralized process-launch boundary.
- A failed launch of an **Installed Desktop Application** is a failed **Panel Action** and does not change the normal **Panel Menu** entry by default.
- A **Desktop Application ID** identifies an **Installed Desktop Application** even when multiple applications share the same display name.
- A **Panel Menu** may use secondary metadata to disambiguate **Installed Desktop Applications** with the same display name.
- **Installed Desktop Applications** may supply best-effort **Panel Menu Entry Icons**, but missing icons do not prevent display or launch.
- Invalid **Panel Menu Entry Icons** from installed application metadata are treated as missing icons, not **Configuration Errors**.
- An installed-applications **Panel Menu Source** presents **Installed Desktop Applications** as a flat list by default.
- **Menu Search** uses fuzzy matching for configured **Panel Menus**, installed-applications **Panel Menu Sources**, **Dmenu Panel Menu Sources**, and configured searchable **SNI Menus**.
- **Menu Search** uses the same matching, ranking, and nested-entry behavior everywhere it is enabled.
- **Menu Search** uses fuzzy matching as the default behavior rather than offering a substring-search mode.
- **Menu Search** fuzzy matching requires query characters to appear in order.
- **Menu Search** is ASCII case-insensitive.
- **Menu Search** splits search text on whitespace and requires every query token to match.
- **Menu Search** ranks matching entries by match quality and uses source order as the tie-breaker.
- **Menu Search** ranks substring matches ahead of non-contiguous fuzzy matches when other priority rules are equal.
- **Menu Search** preserves source order when the search text has no non-whitespace tokens.
- **Menu Search** returns all matching entries rather than applying a hidden result cap.
- **Menu Search** ranks visible leaf labels ahead of secondary metadata, and secondary metadata ahead of breadcrumb-only ancestor matches.
- **Menu Search** can match breadcrumb ancestor labels for nested entries as lowest-priority text.
- **Menu Search** uses Niribar-owned matching behavior rather than exposing a third-party matcher as domain behavior.
- **Menu Search** for **Installed Desktop Applications** matches application names, descriptions, generic names, and keywords.
- A selectable menu may allow a **Custom Menu Value** when its source or configuration enables typed values.
- A **Custom Menu Value** appears after fuzzy-matched entries, or first when there are no matching entries.
- Activating a **Selectable Menu Session** produces a generic selected entry; the owning runtime interprets that selected entry as a **Panel Action**, **Installed Desktop Application** launch, **Dmenu Selection Request** response, or **SNI Menu** item activation.
- A **Selectable Menu Session** owns both pointer hover and keyboard focus for its entries.
- Niribar has at most one active **Selectable Menu Session** at a time.
- A dynamic **Panel Menu Source** is refreshed when its **Panel Menu** opens and remains stable while that **Panel** is open.
- A selectable menu may contain nested items, but Niribar displays one active menu level at a time unless **Menu Search** is active.
- **Menu Search** matches nested items and displays matching results with breadcrumb context.
- A searched **Panel Menu** entry keeps its own **Panel Menu Entry Icon**; breadcrumb ancestors do not provide the result icon.
- Activating a command item in a **Panel Menu** closes the **Panel** by default.
- Activating a command from module panel content does not close the **Panel** by default.
- A **Global Panel Toggle** closes the currently open **Panel**, or opens the default **Panel** for the focused **Output** when no **Panel** is open.
- If no focused **Output** is known, a **Global Panel Toggle** opens one deterministic **Bar Surface** default **Panel**, not one **Panel** per **Bar Surface**.
- **Dmenu Selection Mode** is a standalone CLI lifecycle from the caller's perspective, but the running Niribar instance owns the visible selector surface.
- **Dmenu Selection Mode** sends a **Dmenu Selection Request** to the running Niribar instance.
- **Dmenu Selection Mode** fails without opening a fallback UI when no running Niribar instance is available to handle the **Dmenu Selection Request**.
- **Dmenu Selection Mode** reads its standard input completely before sending one bounded **Dmenu Selection Request**.
- **Dmenu Selection Mode** aims for dmenu-style selection behavior, not full dmenu flag compatibility.
- **Dmenu Selection Mode** accepts a prompt and custom input delimiter as public CLI options.
- **Dmenu Selection Mode** accepts prompt text with `-p` or `--prompt`.
- **Dmenu Selection Mode** uses newline as the default input delimiter.
- **Dmenu Selection Mode** delimiter options interpret common escapes such as `\n`, `\t`, `\0`, and `\\`.
- **Dmenu Selection Mode** delimiter options apply only to plain **Dmenu Entry Streams**; combining a custom delimiter with rich JSONL input is a usage error.
- The **Dmenu Selection Mode** prompt becomes a **Panel Menu Prompt** for the request-owned **Panel Menu**.
- **Dmenu Selection Mode** treats the custom delimiter as input parsing only; successful output is always the selected entry followed by a newline.
- Empty input is valid in **Dmenu Selection Mode**.
- **Dmenu Selection Mode** preserves duplicate input entries in display order.
- **Dmenu Selection Mode** preserves duplicate **Rich Dmenu Entries** in display order, regardless of shared labels or values.
- **Dmenu Selection Mode** allows the user to submit a **Custom Menu Value** by default.
- **Dmenu Selection Mode** can be explicitly restricted to provided entries with `--no-custom`.
- Empty input with `--no-custom` offers no selectable entries and can only be cancelled.
- When **Dmenu Selection Mode** has a focused matching entry, normal activation returns that focused entry.
- Submitting a **Custom Menu Value** uses a distinct custom-value affordance.
- **Dmenu Selection Mode** writes the selected entry and does not run configured **Panel Actions**.
- **Dmenu Selection Mode** exits successfully only when the user selects or submits a value.
- Cancelling **Dmenu Selection Mode** exits distinctly from CLI usage errors and IPC/request failures.
- **Dmenu Selection Mode** has no default timeout while waiting for user selection.
- If the running Niribar instance disconnects before completing a **Dmenu Selection Request**, the CLI treats it as an IPC/request failure.
- A **Dmenu Selection Request** opens on the focused **Output** when known, or on one deterministic **Bar Surface** when no focused **Output** is known.
- Opening a **Dmenu Selection Request** closes any existing **Panel** or **Menu**.
- A running **Dmenu Selection Request** rejects additional **Dmenu Selection Requests** until it completes.
- A **Dmenu Selection Request** cancels when its **Panel** loses focus, consistent with other **Panels**.
- Clearing the **Panel Runtime** for an active **Dmenu Selection Request** returns a cancelled response to the requesting CLI instead of silently dropping the responder.
- A **Dmenu Selection Request** is not a configured **Panel Action**.
- A **Dmenu Selection Request** is displayed as a **Panel Menu** with a **Dmenu Panel Menu Source**.
- A **Dmenu Panel Menu Source** returns its activated value to the requesting CLI process instead of running a configured **Panel Action**.
- A **Dmenu Panel Menu Source** initially treats each input entry as both the visible label and returned value.
- Plain **Dmenu Entry Streams** treat each record as both the visible label and returned value.
- Rich **Dmenu Entries** keep the candidate record boundary line-oriented while allowing the visible label and returned value to differ.
- A value-only **Rich Dmenu Entry** uses its returned value as its visible label.
- Selecting a **Rich Dmenu Entry** returns its value to the caller; if no separate value is present, it returns the visible label.
- A custom typed value in **Dmenu Selection Mode** is returned as typed and is not a **Rich Dmenu Entry**.
- **Dmenu Selection Mode** keeps its selection-value semantics when using **Menu Search**.
- **Dmenu Selection Mode** search matches rich entry labels and keywords; it does not match opaque returned values unless no label is provided.
- Rich **Dmenu Entries** are selectable leaf candidates, not configured **Panel Menu** items; they do not define **Panel Actions** or child entries.
- Rich **Dmenu Entry** icons use the same XDG icon-theme-name constraint as other **Panel Menu Entry Icons**.
- A malformed **Rich Dmenu Entry** is a **Dmenu Selection Mode** input error, not a missing icon or empty entry.
- A valid **Rich Dmenu Entry** icon that cannot be resolved in the current icon theme is treated as a missing **Panel Menu Entry Icon**.
- A **Dmenu Selection Request** with at least one rich entry icon renders **Panel Menu Entry Icons** for that request.
- **Keyboard Focus** belongs to a **Bar Surface**.
- **Keyboard Focus** navigation includes visible targets that are actionable or inspectable.
- A target with a hover **Tooltip** is inspectable and therefore participates in **Keyboard Focus** navigation.
- A target that exposes a **Menu** participates in **Keyboard Focus** navigation.
- A **Bar Surface** with no visible actionable or inspectable targets has no **Keyboard Focus** target.
- Public configuration names **Zones** as `start`, `center`, and `end`.
- A missing **Compositor Integration** makes only the dependent **Modules** unavailable; it does not prevent **Bar Surface** creation.
- Compositor-backed **Modules** use generic names for portable user-facing concepts and compositor-specific names only for non-portable concepts.
- Workspace-capable **Modules** render **Workspaces** relevant to the **Bar Surface**'s **Output** by default.
- A focused-window **Module** shows the **Focused Window** on the focused **Output** and the **Active Window** for non-focused **Outputs**.
- **Start** maps to top for vertical **Bars** and left for horizontal **Bars**.
- **End** maps to bottom for vertical **Bars** and right for horizontal **Bars**.
- A top or bottom **Edge** gives a **Bar** horizontal **Orientation**.
- A left or right **Edge** gives a **Bar** vertical **Orientation**.
- An **Unavailable Module** renders no visible content and leaves no gap in the **Bar Surface**.
- A **Bar Surface** reflows visible targets when a **Module** becomes unavailable or recovers.
- An **Unavailable Module** can recover automatically when its data source or **Compositor Integration** becomes available.
- An **Unavailable Module** has no **Tooltip** on the **Bar Surface**.
- An **Unavailable Module** is skipped by **Keyboard Focus** navigation.
- An **Empty Module** renders no visible content and leaves no gap in the **Bar Surface**.
- An **Empty Module** is not a runtime failure.
- An **Empty Module** has no **Tooltip** on the **Bar Surface** and is skipped by **Keyboard Focus** navigation.
- A **Configuration Error** is reported actionably and prevents the invalid configuration from being used.
- An unknown **Module** reference in configuration is a **Configuration Error**.
- A valid **Module** with a missing runtime dependency becomes an **Unavailable Module**, not a **Configuration Error**.
- Referencing a built-in **Module** with meaningful, safe **Global Defaults** is valid even when that **Module** is not explicitly declared.
- **Modules** that require user-provided commands or labels must be explicitly configured.

## Example dialogue

> **Dev:** "If the user defines one **Bar** and connects two **Outputs**, do we create two **Bar Surfaces**?"
> **Domain expert:** "Yes, if that **Bar** targets both **Outputs**."
>
> **Dev:** "Is it an error if `bar \"dock\"` targets `DP-1` while `DP-1` is disconnected?"
> **Domain expert:** "No — the **Bar** produces zero **Bar Surfaces** until a matching **Output** becomes eligible."
>
> **Dev:** "Does each **Bar Surface** need its own **Module** configuration?"
> **Domain expert:** "No — the **Module** is configured once, but its **Module Instance** can render output-sensitive information per **Bar Surface**."
>
> **Dev:** "Should a horizontal **Bar** still describe its first **Zone** as top?"
> **Domain expert:** "No — the domain term is **Start**. Existing `top` config syntax is legacy wording for the first **Zone**."
>
> **Dev:** "Can a **Bar** be anchored to the left **Edge** but use horizontal **Orientation**?"
> **Domain expert:** "No — **Orientation** is derived from **Edge**."
>
> **Dev:** "If a default **Bar** and an output-targeted **Bar** both match `DP-1`, does one replace the other?"
> **Domain expert:** "No — **Output Targets** are additive, so `DP-1` gets both **Bar Surfaces**."
>
> **Dev:** "Does `output \"!DP-1\"` use a wildcard or pattern?"
> **Domain expert:** "No — it is a negative exact **Output Target** that applies to every **Eligible Output** except exact `DP-1`."
>
> **Dev:** "Can a **Bar** target `DP-1` and `HDMI-A-1`?"
> **Domain expert:** "Yes — use one positive **Output Target** list, such as `output \"DP-1\" \"HDMI-A-1\"`."
>
> **Dev:** "Can a **Bar** exclude both `eDP-1` and `HDMI-A-1`?"
> **Domain expert:** "Yes — use one negative **Output Target** list, such as `output \"!eDP-1\" \"!HDMI-A-1\"`."
>
> **Dev:** "Do we require Niri IPC before creating **Bar Surfaces**?"
> **Domain expert:** "No — **Eligible Outputs** come from Wayland and configuration. Niri IPC can enrich module data, but it is not required for surface creation."
>
> **Dev:** "Is Niribar permanently limited to Niri?"
> **Domain expert:** "No — Niri is the first **Compositor Integration**, but future integrations may support other compositors."
>
> **Dev:** "Should Niri workspaces be configured as `niri-workspaces`?"
> **Domain expert:** "No — **workspaces** is a portable user-facing concept, so the **Compositor Integration** supplies the backend."
>
> **Dev:** "If NetworkManager is unavailable, should the network **Module** show an error badge?"
> **Domain expert:** "No — it becomes an **Unavailable Module** and renders no visible content."
>
> **Dev:** "If Niri IPC appears after startup, should the workspaces **Module** stay hidden?"
> **Domain expert:** "No — **Unavailable Module** is a current state, not a terminal failure."
>
> **Dev:** "If no window is open on an **Output**, is the focused-window **Module** unavailable?"
> **Domain expert:** "No — it is an **Empty Module** because the compositor data source is available but has no current content to show."
>
> **Dev:** "If the tray host is available but no tray icons are registered, is the tray unavailable?"
> **Domain expert:** "No — it is an **Empty Module** because its data source is available but has no current content."
>
> **Dev:** "If a script command succeeds but renders empty output, should it reserve space?"
> **Domain expert:** "No — it is an **Empty Module** and leaves no gap."
>
> **Dev:** "If a script command fails after previously showing data, should Niribar keep the old value?"
> **Domain expert:** "No — the script becomes an **Unavailable Module** for that refresh."
>
> **Dev:** "Can the user hover an **Unavailable Module** to see why it disappeared?"
> **Domain expert:** "No — an **Unavailable Module** has no visible target and therefore no **Tooltip** on the **Bar Surface**."
>
> **Dev:** "Can keyboard navigation focus an **Unavailable Module**?"
> **Domain expert:** "No — unavailable modules have no visible target and are skipped by **Keyboard Focus** navigation."
>
> **Dev:** "Should `edge \"diagonal\"` become an **Unavailable Module**?"
> **Domain expert:** "No — invalid user configuration is a **Configuration Error**, not runtime unavailability."
>
> **Dev:** "Should `start \"clcok\"` be skipped silently?"
> **Domain expert:** "No — an unknown **Module** reference is a **Configuration Error**."
>
> **Dev:** "If the user configures `battery` on a desktop without UPower battery data, is the config invalid?"
> **Domain expert:** "No — `battery` is a valid **Module**, so it becomes an **Unavailable Module** when its runtime data is missing."
>
> **Dev:** "Can a **Bar** reference `clock` without a separate `clock` configuration block?"
> **Domain expert:** "Yes — built-in **Modules** with meaningful, safe **Global Defaults** can be synthesized from their defaults."
>
> **Dev:** "Can a **Bar** reference `script` without defining its command?"
> **Domain expert:** "No — **Modules** that require user-provided commands or labels must be explicitly configured."
>
> **Dev:** "Is a polling refresh a **Module Action**?"
> **Domain expert:** "No — a **Module Action** is an intentional user-triggered effect, such as running a command, focusing a workspace, or changing volume."
>
> **Dev:** "Should a failed button command turn the button into an error indicator?"
> **Domain expert:** "No — failed **Module Actions** do not change the normal **Module** display by default."
>
> **Dev:** "If a button command would fail when clicked, should the button become unavailable?"
> **Domain expert:** "No — a valid button stays visible; command failure after activation is a failed **Module Action**."
>
> **Dev:** "Is a **Tooltip** a **Module Action**?"
> **Domain expert:** "No — a **Tooltip** is supplemental information shown for a **Module**."
>
> **Dev:** "Does the tray parent own one **Tooltip** for all tray icons?"
> **Domain expert:** "No — each visible tray icon child **Module Instance** owns the **Tooltip** for its target."
>
> **Dev:** "Is a tray context menu a **Tooltip**?"
> **Domain expert:** "No — a tray context menu is a **Menu**; selecting a menu item triggers a **Module Action**."
>
> **Dev:** "Can a **Tooltip** stay open behind a tray **Menu**?"
> **Domain expert:** "No — opening a **Menu** closes open **Tooltips** on that **Bar Surface**."
>
> **Dev:** "Can one **Bar Surface** show tooltips for two modules at once?"
> **Domain expert:** "No — a **Bar Surface** shows at most one **Tooltip** at a time."
>
> **Dev:** "Can two tray **Menus** be open on two outputs at the same time?"
> **Domain expert:** "No — Niribar shows at most one **Menu** at a time."
>
> **Dev:** "If the user focuses a module on `DP-1`, should the same **Module** be focused on `eDP-1`?"
> **Domain expert:** "No — **Keyboard Focus** belongs to the individual **Bar Surface**."
>
> **Dev:** "Should a display-only clock be keyboard-focusable?"
> **Domain expert:** "Only if it exposes an inspectable affordance such as a **Tooltip**."
>
> **Dev:** "Can pointer hover show a **Tooltip** for a target that keyboard navigation cannot reach?"
> **Domain expert:** "No — hover tooltips and keyboard inspectability use the same target model."
>
> **Dev:** "If the user opens a **Panel** from the **Bar Surface** on `DP-1`, should the matching surface on `eDP-1` open a panel too?"
> **Domain expert:** "No — **Panel** state belongs to the originating **Bar Surface**."
>
> **Dev:** "Is clicking the bar background a **Module Action**?"
> **Domain expert:** "No — it is a **Bar Surface Interaction** and may trigger a **Panel Action** when configured."
>
> **Dev:** "Is `niribar --toggle-panel` a configured **Panel Action** trigger?"
> **Domain expert:** "No — it is a **Global Panel Toggle** routed through IPC, not a direct pointer or focused-key trigger."
>
> **Dev:** "What should `niribar --toggle-panel` affect?"
> **Domain expert:** "It should close the current **Panel** when one is open. Otherwise it should open one default **Panel** on the focused **Output** when known, or one deterministic default **Panel** if no focused **Output** is known."
>
> **Dev:** "Is `niribar --dmenu` another way to open a configured **Panel Menu**?"
> **Domain expert:** "No — **Dmenu Selection Mode** is a one-shot selector that returns the chosen input entry to its caller."
>
> **Dev:** "Can two **Bar Surfaces** have **Panels** open at the same time?"
> **Domain expert:** "No — Niribar shows at most one **Panel** at a time."
>
> **Dev:** "Does opening a **Panel** make the **Bar Surface** wider or taller?"
> **Domain expert:** "No — the **Bar Surface** keeps its normal form; the **Panel** is a separate overlay surface."
>
> **Dev:** "If a **Module** opens a **Panel**, should the source still be visible on the **Bar Surface**?"
> **Domain expert:** "Yes — the **Panel Origin** remains visually marked while the **Panel** is open."
>
> **Dev:** "Can a **Bar**'s default **Panel** show modules that are not in the normal **Bar** layout?"
> **Domain expert:** "Yes — a **Panel** module layout is independent from the normal **Bar** module layout."
>
> **Dev:** "Can a **Panel Menu** reuse the same **Panel Action** language as a **Module**?"
> **Domain expert:** "Yes — **Panel Actions** are shared across bar triggers, module triggers, and panel menu items."
>
> **Dev:** "Should hover open a **Panel**?"
> **Domain expert:** "No — opening a **Panel** is an explicit configured action, not a hover-triggered state."
>
> **Dev:** "Should Niribar reject two **Bar Surfaces** on the top **Edge** of `DP-1`?"
> **Domain expert:** "No — it is valid, but the configuration is responsible for avoiding unusable overlap."
>
> **Dev:** "Will Niribar automatically stack or offset two top-edge **Bar Surfaces**?"
> **Domain expert:** "No — same-edge overlap is not resolved automatically."
>
> **Dev:** "Can `clock` appear in both `main` and `secondary` **Bars**?"
> **Domain expert:** "Yes, but it should not be placed twice within the same **Bar**."
>
> **Dev:** "Did the user configure each tray icon as a **Module**?"
> **Domain expert:** "No — the tray is a **Dynamic Module** that produces child **Module Instances** from session bus state."
>
> **Dev:** "Should keyboard focus stop on each workspace item?"
> **Domain expert:** "Yes — child **Module Instances** are individually navigable when they represent distinct user targets."
>
> **Dev:** "Should the public config keep `top` and `bottom` zone names?"
> **Domain expert:** "No — Niribar is not released yet, so public config should use `start`, `center`, and `end` without deprecated aliases."
>
> **Dev:** "Does every named **Bar** need to repeat style and panel settings?"
> **Domain expert:** "No — **Global Defaults** keep named **Bars** lightweight, and each **Bar** can override only what differs."
>
> **Dev:** "If the config declares `bar \"main\"`, should Niribar also create the **Implicit Bar**?"
> **Domain expert:** "No — the **Implicit Bar** exists only when no **Bars** are declared."
>
> **Dev:** "Can `bar \"compact\"` change the `clock` format only for that **Bar**?"
> **Domain expert:** "No — `clock` is configured globally by name. Define another clock **Module** if the behavior differs."
>
> **Dev:** "Are `local-clock` and `utc-clock` the same **Module** because both are clock modules?"
> **Domain expert:** "No — they share a **Module Kind**, but they have different **Module Names** and do not share runtime state."
>
> **Dev:** "Can a script **Module** be named `clock`?"
> **Domain expert:** "No — built-in **Module Names** are reserved."
>
> **Dev:** "Can a user-declared **Module** redefine the built-in `clock` name?"
> **Domain expert:** "Yes, but only as a `clock` **Module**. It can override the clock **Global Defaults**, but it cannot change `clock` to another **Module Kind**."
>
> **Dev:** "Can a **Bar** place `utc` without declaring a module named `utc`?"
> **Domain expert:** "No — `utc` is not a built-in **Module Kind**, so it must resolve to a declared **Module Name**."
>
> **Dev:** "Should `DP-1` override the modules of `bar \"main\"`?"
> **Domain expert:** "No — define another named **Bar** targeting `DP-1` when output-specific behavior differs."
>
> **Dev:** "Should Niribar keep per-output **Bar** override syntax as an alternative?"
> **Domain expert:** "No — Niribar is not released yet, so remove that syntax and use named **Bars** with **Output Targets** instead."
>
> **Dev:** "If a laptop panel is excluded, can any **Bar** still target it?"
> **Domain expert:** "No — an **Excluded Output** is not eligible for any **Bar Surface**."

## Flagged ambiguities

- "bar" was used to mean both a configured **Bar** and a live **Bar Surface**; resolved: these are distinct concepts.
- "module" was used to mean both configured content and runtime state; resolved: **Module** is the configured content unit, while **Module Instance** is runtime state and behavior.
- "top" and "bottom" describe physical positions, but the domain uses logical **Start** and **End** zones so the same language works for vertical and horizontal **Bars**.
