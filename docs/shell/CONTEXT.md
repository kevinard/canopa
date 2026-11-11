# Canopa Shell

Canopa Shell is Canopa's desktop shell for the Niri Wayland compositor. It renders configurable bar surfaces on Wayland outputs and hosts panels, drawers, menus, and the Notification Center. Modules show information or trigger actions.

## Language

### Bars and Outputs

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
The stable planning and reconciliation identity for a **Bar Surface**: the **Bar Name** plus an **Output**.
_Avoid_: Output/bar tuple, window ID

**Bar Surface Midpoint**:
The visual midpoint of a **Bar Surface** along its **Orientation**.
_Avoid_: Screen center, Output center, remaining-space center

**Bar Surface Plan**:
A runtime description of desired **Bar Surfaces** derived from configured **Bars** and current **Eligible Outputs** before live window reconciliation.
_Avoid_: Desired windows, surface list

**Bar Surface Physical Plan**:
The physical layer-shell contract for a planned **Bar Surface**, including its **Edge** anchor, **Layer**, logical size, and exclusive zone before a live surface is opened. The target **Output** comes from the **Bar Surface Identity**.
_Avoid_: Window settings, layer-shell config

**Bar Surface Registry**:
The runtime owner of live **Bar Surface** records and passive **Tooltip** metadata associated with **Bar Surfaces**.
_Avoid_: Overlay lifecycle, surface manager

**Output**:
A Wayland display target that can host zero or more **Bar Surfaces**.
_Avoid_: Screen, monitor, display

**Output Target**:
An exact include-list or exclude-list expression that selects which **Eligible Outputs** a **Bar** should render on.
_Avoid_: Output override, monitor filter

**Eligible Output**:
A currently active Wayland **Output** that is not excluded by configuration.
_Avoid_: Niri output, connected output

**Wayland Output Adapter**:
The application-owned adapter over Wayland/layer-shell output events that supplies **Output** add, remove, and name facts used to derive **Eligible Outputs** for **Bar Surface** planning.
_Avoid_: Niri output source, workspace output discovery

**Wayland Output Snapshot**:
The current set of named Wayland **Outputs** emitted by the **Wayland Output Adapter** whenever output availability or output naming changes.
_Avoid_: output added event, output removed event

**Excluded Output**:
An **Output** that configuration prevents Canopa Shell from rendering on.
_Avoid_: Disabled bar, ignored monitor

**Edge**:
The side of an **Output** where a **Bar Surface** is anchored: top, bottom, left, or right.
_Avoid_: Orientation, side

**Edge Span**:
The length a **Bar Surface** occupies along its **Edge** after accounting for other **Bar Surfaces** on the same **Output**.
_Avoid_: Full width, full height

**Layer**:
The compositor stacking layer used by a **Bar Surface**. Allowed values are background, bottom, top, and overlay. The default is top.
_Avoid_: Popup layer, z-index

**Orientation**:
The layout direction of a **Bar**, derived from its **Edge**.
_Avoid_: Edge

### Wallpapers

**Wallpaper Selection**:
Shell-owned durable references to immutable background and lockscreen images.
Both destinations change together when requested; bar configuration is independent.

**Wallpaper Surface**:
An input-transparent Iced background layer on a named **Output**, managed separately
from **Bar Surfaces**. It covers the output without reserving workarea.

The private Wallpaper Owner prepares bounded PNG descriptors off the UI thread,
persists selections and installs background pixels in App. `canopa-shell lock`
pins the selected lock image and executes Swaylock through the session-owned
launch path. See [ADR0037](../adr/0037-shell-wallpaper-owner.md). The public
Wallpaper portal is not advertised while native acceptance and review are pending.

### Modules

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
The runtime state and behavior of a **Module** while Canopa Shell is running.
_Avoid_: Module, widget instance

**Module Gap**:
The visual separation between adjacent visible **Modules** on a **Bar Surface**.
_Avoid_: Separator, border

**Module Runtime**:
The app-owned runtime module that owns **Module Instances**, per-instance state, **Module Actions**, origin metadata, interaction profiles, rendering, **Tooltip** values, **Module Data Source** mapping, **Module Detail Session** routing, and dynamic-child storage behavior.
_Avoid_: Module storage, widget runtime

**Module Action**:
An intentional user-triggered effect caused by interacting with a **Module**.
_Avoid_: Event, subscription, refresh, panel action when the action is not module-scoped

**Module Interaction**:
A pointer or keyboard input delivered to a **Module Instance** before it resolves into a **Module Action**, **Menu**, or module-owned effect.
_Avoid_: Module action, app event, key binding

**Unavailable Module**:
A **Module** whose data source cannot currently provide useful data.
_Avoid_: Failed module, hidden module

**Empty Module**:
A **Module** whose data source is available but has no current content to show.
_Avoid_: Unavailable module, failed module

**Format Template**:
A configured text template that controls which **Module** values appear in a render mode and in what order, including whether an icon placeholder is part of the visible content.
_Avoid_: Label format, display string

**Content Rotation**:
A presentation choice that changes the direction in which a **Module**'s visible content is drawn without changing the **Bar**'s **Orientation**, **Edge**, or placement.
_Avoid_: Orientation, rotated bar, edge rotation

**Application Icon**:
The icon associated with the application that owns a compositor window, used when a **Module** represents that window rather than a generic module glyph.
_Avoid_: Window icon, module icon

Local icon lookup and Center raster preparation use shared `canopa-assets`
mechanisms. Canopa Shell owns SVG rendering, process-lifetime icon caches, Center image
limits and worker admission; see [shared asset ownership](../adr/0005-shared-assets.md).

### Idle inhibition

**Idle Inhibitor Module**:
The built-in `idle-inhibitor` **Module** controls one manual, shell-wide
keep-awake preference, initially off. Pointer and keyboard activation toggle it.
Its app-owned **Module Data Source** serializes Wayland adapter requests and shares
acknowledged state across **Module Instances**. The adapter owns one inhibitor
per live **Bar Surface** and destroys it with the surface. See [ADR 0017](adr/0017-manual-idle-inhibitor.md).
_Avoid_: Suspend blocker, idle daemon

### Notifications

**Notification Module**:
The built-in `notifications` **Module** displaying Notify's attention count, DND and availability. It shares one **Module Data Source**, opens the built-in Center provider on primary activation and requests an atomic DND toggle on secondary activation. It stays visible when unavailable and replaces its snapshot after reconnect; Notify owns history and policy.
_Avoid_: Notification daemon, unread counter

### Audio

**Audio Panel Provider**:
The built-in `audio` **External Panel Provider** owning output/input device volume,
mute and default-device selection. It shares PulseAudio connection mechanics with
the volume **Module**, while its controls and observation end with its **Panel**.
_Avoid_: Audio daemon, mixer when referring to application streams

The bar's default-output observer sleeps until state changes or cancellation.
See [ADR 0022](adr/0022-event-driven-volume-observation.md).

**Audio Device**:
A PulseAudio output sink or non-monitor input source, identified by its live
connection generation, server cookie, kind, index and name.
_Avoid_: Output when the term means a Wayland display

### Media

**Media Module**:
The built-in `media` **Module** showing the selected MPRIS player's metadata and
playback state. Its shared **Module Data Source** owns automatic or manual player
selection and dispatches supported playback controls. Existing players own playback.
_Avoid_: Media player when referring to the shell, audio device

**Media Panel Provider**:
The built-in `media` **External Panel Provider** showing track information, player
selection and supported transport controls. It shares the shell's selected player
through the source-owned session-bus interface. See [ADR 0018](adr/0018-mpris-media-controls.md).
_Avoid_: Playback engine, media daemon

### Device keys

**Device Key**:
A volume, microphone, brightness, keyboard-backlight or media key, run as a
`canopa-shell` command that the compositor binds. The running Shell executes it
and shows an **Indicator**; without a Shell the command runs it alone. See
[ADR 0025](adr/0025-device-keys-and-on-screen-display.md).
_Avoid_: Hotkey, media key when the term includes volume and brightness

**On-screen Display (OSD)**:
The passive overlay surface on the focused **Output** that shows one
**Indicator**. It is not part of the **Foreground**: it takes no keyboard focus,
lets pointer input through and never dismisses a **Panel**.
_Avoid_: Popup, notification, banner

**Indicator**:
What the **On-screen Display** shows for one channel (volume, microphone,
brightness, keyboard brightness): an icon, the device, a level and a value, or
"Unavailable". A new **Indicator** replaces the visible one.
_Avoid_: Toast, level popup

**Outside Change**:
A level change the Shell did not make, from another application or the firmware.
It shows an **Indicator** once it settles; the first reading is only a baseline.
_Avoid_: External event when the term means a key press

### Network

**Network Status Integration**:
An app-owned integration that reports current connectivity and the active connection label for the network **Module**.
_Avoid_: Wi-Fi backend when no scan or connection behavior is involved

**Wi-Fi Control Integration**:
An external-provider-owned integration that discovers nearby **Wi-Fi Networks** and starts connection attempts from **External Panel Content**.
_Avoid_: Network status backend, networkd path

**Wi-Fi Panel Provider**:
An **External Panel Provider** that owns **Wi-Fi Control Integration** for the network **Module**.
_Avoid_: Network provider when the provider only owns wireless control

**Wi-Fi Network**:
A discoverable wireless network shown as a selectable target in **External Panel Content**. One **Wi-Fi Network** may be advertised by multiple physical access points.
_Avoid_: Access point when referring to the user-facing network choice

**Personal Wi-Fi Network**:
A wireless connection target that can be joined with a single **Network Credential**.
_Avoid_: Enterprise Wi-Fi Network when additional identity, certificate, or EAP settings are required

**Hidden Wi-Fi Network**:
A wireless connection target identified by a user-provided network name rather than by a **Wi-Fi Scan** result.
_Avoid_: Wi-Fi Network when the target was not discovered

**Known Wi-Fi Network**:
A **Wi-Fi Network** with saved connection material available to the active **Wi-Fi Control Integration**.
_Avoid_: Saved SSID, configured profile

**Active Wi-Fi Network**:
The **Wi-Fi Network** that the active **Wi-Fi Control Integration** reports as currently connected.
_Avoid_: Current network when the connection may be non-wireless

**Wi-Fi Scan**:
A refresh of discoverable **Wi-Fi Networks** performed by the active **Wi-Fi Control Integration**.
_Avoid_: Network status update, connectivity check

**Wi-Fi Connection Attempt**:
A user-initiated attempt to connect to a selected **Wi-Fi Network** through the active **Wi-Fi Control Integration**.
_Avoid_: Network status update, module action

**Wi-Fi Network State**:
The user-facing status of a **Wi-Fi Network** row, such as known, active, connecting, or failed.
_Avoid_: Connectivity when referring to a row-level connection state

**Network Credential**:
A user-provided secret used to connect to a **Wi-Fi Network** that is not a **Known Wi-Fi Network**.
_Avoid_: Password when referring to stored or submitted connection material generally

**Network Credential Prompt**:
An inline state of **External Panel Content** that uses **External Text Input** to collect a **Network Credential** for a selected personal **Wi-Fi Network**.
_Avoid_: Password dialog, credential popup, separate overlay

### Dynamic Modules and Data Sources

Configured names and child identities are structurally distinct. A child identity contains its configured parent and typed runtime key; formatting a name never creates a child identity. Parent ownership is stored once in the identity. See [ADR 0019](adr/0019-structural-module-identities.md).

**Dynamic Module**:
A **Module** that produces runtime child **Module Instances** from external state.
_Avoid_: Generated module, synthetic module

**Dynamic Module Child Key**:
The stable runtime identity used by the **Dynamic Module Lifecycle** to match one desired child **Module Instance** with an existing child **Module Instance** for the same **Dynamic Module**.
_Avoid_: Generated ID, child widget key

**Dynamic Module Family**:
A specific **Dynamic Module** behavior family with a parent **Module Kind**, child **Module Kind**, **Dynamic Module Child Key** rule, child construction rule, and child update policy. Examples: tray icons and workspace items.
_Avoid_: Generic dynamic module helper, generated module type

**Dynamic Module Family Input**:
A normalized, family-specific input derived from parent **Module** configuration and inherited runtime metadata before the **Dynamic Module Lifecycle** constructs or updates child **Module Instances**.
_Avoid_: Raw module config, family options, constructor params

**Dynamic Module Child Fact**:
A source-shaped runtime fact used by the **Dynamic Module Lifecycle** to construct, update, rebuild, or remove one child **Module Instance** for a **Dynamic Module Family**.
_Avoid_: Child config, generated child state

**Dynamic Module Reconciliation Plan**:
A pure runtime description of child **Module Instance** changes, inherited metadata, and stale **Keyboard Focus** or hover cleanup targets returned by the **Dynamic Module Lifecycle**.
_Avoid_: App mutation, callback result, generated diff

**Dynamic Module Runtime Snapshot**:
A narrow, read-only snapshot of app-owned runtime facts the **Dynamic Module Lifecycle** needs to reconcile **Dynamic Modules**. Layout expansion reads indexed identities without constructing this snapshot.
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

### Layout

**Zone**:
A logical placement group within a **Bar**, named **Start**, **Center**, or **End**.
_Avoid_: Top, bottom, left, right

**Center Zone**:
The **Zone** whose content is visually aligned to the **Bar Surface Midpoint**.
_Avoid_: Remaining-space center, balanced zone

### Interaction and Overlays

**Keyboard Focus**:
The current **Module** or child **Module Instance** target selected for keyboard interaction on a **Bar Surface**.
_Avoid_: Global focus, compositor focus

**Bar Surface Interaction**:
A user interaction with a **Bar Surface** that is not targeted at a **Module**.
_Avoid_: Module action

**Tooltip**:
Supplemental information shown for a **Module** or child **Module Instance** on a **Bar Surface**.
_Avoid_: Popup action, module display

**Menu**:
An interactive popup exposed by a **Module** or child **Module Instance**.
_Avoid_: Tooltip, action, launcher

**SNI Menu**:
A **Menu** exposed by a StatusNotifierItem through DBusMenu and rendered by Canopa Shell as a popup.
_Avoid_: Panel Menu, launcher, configured menu

**Selectable Search**:
The user-facing matching behavior that filters or ranks selectable entries from typed input.
_Avoid_: Filter when referring to ranked matching behavior

**Menu Search**:
**Selectable Search** applied to selectable **Menu** or **Panel Menu** entries.
_Avoid_: Panel Menu Search when the behavior is not panel-only

**Custom Menu Value**:
A user-submitted typed value returned by a selectable menu instead of an existing entry.
_Avoid_: Dmenu entry when the value was typed by the user, menu item when no configured item exists

**Selectable Menu Session**:
The live runtime interaction for one selectable **Menu** or **Panel Menu**, including entry source, active menu level, optional **Selectable Search** text, focus, hover, optional **Custom Menu Value**, and selection completion.
_Avoid_: Panel Menu Session when the behavior applies beyond panels, popup state when referring to selection behavior

**Foreground**:
The currently active interactive overlay state: no overlay, a **Menu**, or a **Panel**. **Foreground** owns physical **Panel** and **Menu** overlay window identity, the active **Menu**'s **Selectable Menu Session**, the active **Panel Runtime**, replacement, stale close handling, interactive overlay role classification, and app-level **Panel** interaction routes. **Tooltip** is not **Foreground** because it is passive and owned as a **Bar Surface** affordance.
_Avoid_: overlay registry when referring to interactive state, popup when referring to **Panel** or **Menu** interaction

**App Effect**:
A runtime request to perform external work owned by the application adapter, such as opening or removing physical windows, changing keyboard interactivity, launching processes, sending IPC responses, or calling DBus. **App Effects** do not own **Foreground**, **Panel Runtime**, **Bar Surface Registry**, or **Module Runtime** coordination decisions.
_Avoid_: foreground effect, panel action, module effect

### Panels and Panel Menus

**Panel**:
A transient overlay surface opened from a **Bar Surface**. A **Panel** is anchored to its originating **Bar Surface** or **Module** and can render module detail content, a **Bar**'s configured panel module layout, or named **Panel Menu** content.
_Avoid_: Popup when referring to Canopa Shell's generic panel

**Panel Origin**:
The **Bar Surface** or **Module** that triggered the current **Panel**.
_Avoid_: Active module when referring specifically to panel source

**Panel Sizing**:
The content-aware policy that determines how much output space a **Panel** occupies while respecting its **Panel Origin** and content kind.
_Avoid_: Popup size, fixed panel size

**Drawer Mode**:
The relative placement of a vertical drawer and its originating **Bar Surface**.
`overlay` keeps the bar at the edge and opens the drawer beside it; `push-bar`
places the drawer on the edge side and moves the visible bar inward. Both retain
the bar's original exclusive zone, so application windows do not resize.

**Panel Physical Plan**:
A runtime description of the desired physical overlay placement and size for a **Panel** produced by **Panel Sizing** before the application adapter converts it into layer-shell settings.
_Avoid_: Panel window settings, layer-shell settings

**Panel Runtime**:
The internal **Foreground**-owned module for the live content and interaction state of the currently active **Panel**, including **Selectable Menu Session** state for a **Panel Menu**, pending search focus, menu scroll position, **Module Detail Session** state, and any active **Dmenu Selection Request** responder.
_Avoid_: Panel lifecycle, overlay lifecycle, physical Panel window identity

**Module Detail Content**:
Module-owned content rendered when a **Panel** is opened from a specific **Module**.
_Avoid_: Panel menu, popup, separate module when the content belongs to the originating module

**Module Detail Session**:
The transient module-owned state and interaction lifecycle for **Module Detail Content** while its **Panel** is open.
_Avoid_: Panel runtime state, global module state, persisted detail view

**External Panel Provider**:
An out-of-process owner of interactive panel content that sends Canopa Shell a renderable content model and receives user interaction events back.
_Avoid_: Plugin when implying in-process code loading

**External Panel Session**:
The live bidirectional interaction between Canopa Shell and one **External Panel Provider** for one active **Panel**.
_Avoid_: Dmenu Selection Request when the provider can update content over time

**External Panel Content**:
Provider-owned content rendered by Canopa Shell inside a **Panel** from a provider-supplied content model.
_Avoid_: Module Detail Content when the content lifecycle is owned outside Canopa Shell

**External Panel Element ID**:
An opaque provider-supplied identifier attached to an interactive element in **External Panel Content**.
_Avoid_: Module ID, widget ID when referring to provider-owned element identity

**External Selectable List**:
A selectable-list primitive in **External Panel Content** whose entries are supplied by an **External Panel Provider** while Canopa Shell owns selection rendering, focus, scrolling, and **Selectable Search** behavior. An **External Selectable List** may explicitly allow an **External Custom List Value**.
_Avoid_: Panel Menu when entries are provider-owned external content

**External Custom List Value**:
A user-submitted typed value from an **External Selectable List** instead of an existing provider-supplied entry.
_Avoid_: Custom Menu Value when the value belongs to provider-owned external content

**External Slider**:
A bounded integer control declared on a rich item in **External Panel Content**.
The host owns local gesture preview, keyboard focus and native value accessibility;
the provider interprets committed values through its opaque action key.
_Avoid_: Volume widget when referring to the generic host primitive

**External Text Input**:
A text-entry primitive in **External Panel Content**, optionally rendered as secret input, whose value changes are sent to an **External Panel Provider**.
_Avoid_: Search input when the text is provider-owned form data

**Panel Action**:
A typed configured operation triggered by pointer input, focused keyboard input, **Panel Menu** item activation, or IPC. Examples: `open-panel`, `toggle-panel`, `close-panel`, `open-menu`, `toggle-menu`, `open-provider`, `launch`, and `command`.
`open-bar-panel` and `toggle-bar-panel` select the originating Bar Surface's
configured Bar Panel while retaining a Module origin for anchoring and focus return.
_Avoid_: Shell action when the action is not a command

**Global Panel Toggle**:
An IPC-triggered request to toggle the default **Panel** without direct **Bar Surface** interaction context.
_Avoid_: Toggle all, expansion toggle

**Panel Menu**:
An app-owned, named launcher/menu list rendered inside a **Panel**.
_Avoid_: Menu when referring to SNI or other module-owned context menus

**Global Panel Menu Open**:
An IPC request to open a named **Panel Menu** from the running configuration,
without direct **Bar Surface** interaction context.
_Avoid_: Global Panel Toggle, Dmenu Selection Request

**Global Panel Menu Toggle**:
An IPC request to close a named **Panel Menu** if it is open, cancel its pending
opening, or otherwise perform a **Global Panel Menu Open**.
_Avoid_: Global Panel Toggle

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
A user-launchable application discovered from the desktop environment's application metadata. Terminal applications are included and open in a terminal.
_Avoid_: Desktop file, command, package

**Desktop Application ID**:
The stable identifier Canopa Shell uses to distinguish one **Installed Desktop Application** from another.
_Avoid_: Application name, label

### Compositor and Dmenu

**Compositor Integration**:
An optional connection to a Wayland compositor that enriches **Modules** with compositor-specific information or **Module Actions**.
_Avoid_: Runtime requirement, platform

**Niri Integration**:
The **Compositor Integration** that connects Canopa Shell to Niri IPC.
_Avoid_: Niri runtime, Wayland output source

**Workspace**:
A compositor-provided work area shown by workspace-capable **Modules**.
_Avoid_: Niri workspace, desktop

**Compositor Window**:
A window reported by the compositor integration, including its title, owning application identity, focus state, and output/workspace relationships.
_Avoid_: App window, client, Wayland window

**Focused Window**:
The compositor-reported window that currently has global input focus.
_Avoid_: Active window

**Active Window**:
The compositor-reported window active on a specific **Output**.
_Avoid_: Focused window

**Dmenu Selection Mode**:
A one-shot Canopa Shell invocation that reads candidate entries from standard input, shows a searchable selector through the running Canopa Shell instance, writes the selected value to standard output, and exits.
_Avoid_: Dmenu compatibility mode, launcher mode, panel menu mode

**Dmenu Selection Request**:
A single request sent from a standalone **Dmenu Selection Mode** CLI process to the running Canopa Shell instance for interactive selection.
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

### Configuration

**Global Defaults**:
Configuration values used by **Bars** or **Modules** when they do not provide their own values.
_Avoid_: Base bar, inherited config

**Implicit Bar**:
The default **Bar** Canopa Shell uses when configuration declares no **Bars**.
_Avoid_: Default config, fallback surface

**Configured Command**:
A user-declared command value from configuration, either shell text or an explicit argv program and argument list. It can power script content, command-backed **Module Actions**, and command-backed **Panel Actions**.
_Avoid_: Shell command when the value may be argv; action when referring only to the command value

**Configured Panel Provider**:
A configured or built-in command used to launch an **External Panel Provider** for a **Module** or **Panel Action**.
_Avoid_: Panel Action when referring to the provider process lifecycle

**Panel Provider Name**:
The user-facing identifier for a **Configured Panel Provider**.
_Avoid_: Module Name, command name

**Panel Provider Resolution**:
The runtime lookup that maps a **Panel Provider Name** to a user-declared **Configured Panel Provider**, a built-in provider command, or a provider failure.
_Avoid_: Provider registry, command lookup

**Command Policy**:
The configuration-owned policy for **Configured Commands**, including whether shell-form commands are accepted and how long command-backed **Module Actions** and **Panel Actions** may run before Canopa Shell kills and reaps the process. It validates **Configured Panel Providers** but does not define provider session lifetime, **Dmenu Selection Mode** selection waiting, or **Installed Desktop Application** launch behavior.
_Avoid_: Script timeout, desktop launch policy, security mode

**Configuration Error**:
An invalid user configuration that prevents Canopa Shell from constructing the requested model.
_Avoid_: Unavailable module, runtime failure

## Relationships

### Bars and Outputs

- A **Bar** may produce zero, one, or many **Bar Surfaces**.
- **Bar Names** are globally unique, exact, and case-sensitive.
- Duplicate **Bar Names** are **Configuration Errors**.
- **Bar Names** and **Module Names** live in separate namespaces.
- An **Implicit Bar** exists only when configuration declares no **Bars**.
- The **Implicit Bar** uses `main` as its diagnostic **Bar Name**, but `main` is not reserved for declared **Bars**.
- An **Implicit Bar** uses **Global Defaults** and applies to all **Eligible Outputs**.
- A **Bar** with no **Output Target** applies to all **Eligible Outputs**.
- `all` is the default **Output Target** value for a **Bar**.
- For **Bar Surface Plan** purposes, omitted and explicit all-output **Output Targets** are equivalent.
- When **Eligible Outputs** are unknown, **Bar Surface Plan** produces no **Bar Surfaces** until the **Wayland Output Adapter** reports concrete **Outputs**.
- A live **Bar Surface** belongs to exactly one **Bar** and exactly one **Output**.
- A **Bar Surface Plan** is computed before the **Bar Surface Registry** reconciles desired **Bar Surfaces** with live **Bar Surfaces**.
- A **Bar Surface Plan** does not own live windows, overlay metadata, or **Panel Runtime** interaction state.
- The **Bar Surface Registry** owns live **Bar Surface** records and passive **Tooltip** metadata; it does not own **Foreground**, **Panel Runtime**, or physical **Panel** and **Menu** overlay identity.
- A **Bar Surface Physical Plan** does not inset a **Bar Surface** for orthogonal **Bar Surfaces** on the same **Output**; doing so leaves visible corner gaps.
- A **Bar Surface** uses zero layer-shell margin by default.
- A **Bar Surface** uses the **Layer** configured by its **Bar**.
- A **Bar Surface** remains present even when all of its **Modules** are empty or unavailable.
- An **Output** may host zero, one, or many **Bar Surfaces**.
- **Output** names are matched exactly and case-sensitively.
- Canopa Shell creates **Bar Surfaces** only for **Eligible Outputs**.
- An **Excluded Output** is not an **Eligible Output**.
- Global **Excluded Outputs** are named with plain exact **Output** names, not negative **Output Target** syntax.
- An **Excluded Output** name that does not match a currently connected **Output** is valid.
- Global **Excluded Outputs** take precedence over **Output Targets**.
- Duplicate global **Excluded Output** names are **Configuration Errors**.
- The **Wayland Output Adapter** is the source of **Output** discovery for **Eligible Outputs** and **Bar Surface Plan** reconciliation.
- The **Wayland Output Adapter** emits **Wayland Output Snapshots**, not incremental output add/remove events.
- **Niri Integration** does not provide **Output** discovery for **Bar Surface** planning; it may enrich compositor-backed **Modules** after **Bar Surfaces** exist.
- Canopa Shell does not use layer-shell all-screens startup for normal **Bar Surfaces**; each live **Bar Surface** is opened explicitly for one concrete **Output**.
- A positive **Output Target** list matches any listed exact **Output** name.
- A negative **Output Target** list applies to all **Eligible Outputs** except the listed exact **Output** names.
- A single **Output Target** cannot mix positive and negative entries.
- Duplicate entries in one **Output Target** are **Configuration Errors**.
- An **Output Target** that matches no currently **Eligible Outputs** is valid.
- **Output Targets** are additive: if multiple **Bars** match one **Output**, Canopa Shell creates multiple **Bar Surfaces** for that **Output**.
- A **Bar** may override **Global Defaults**.
- Output-specific bar behavior is modeled by defining another named **Bar** with an **Output Target**, not by mutating a **Bar** per output.
- Per-output **Bar** overrides are outside Canopa Shell's intended configuration model.
- Multiple **Bar Surfaces** may share the same **Output** and **Edge**.
- Canopa Shell does not automatically resolve overlap between **Bar Surfaces** that share the same **Output** and **Edge**.
- Canopa Shell creates horizontal **Bar Surfaces** before vertical **Bar Surfaces** for deterministic physical creation order.

### Layout

- A top or bottom **Edge** gives a **Bar** horizontal **Orientation**.
- A left or right **Edge** gives a **Bar** vertical **Orientation**.
- Public configuration names **Zones** as `start`, `center`, and `end`.
- Public configuration rejects physical zone names such as `top` and `bottom`.
- **Start** maps to top for vertical **Bars** and left for horizontal **Bars**.
- **End** maps to bottom for vertical **Bars** and right for horizontal **Bars**.
- A **Module** belongs to exactly one **Zone** within a **Bar**.
- A **Center Zone** aligns to the **Bar Surface Midpoint** for horizontal and vertical **Bar Surfaces**.
- Multiple visible **Modules** in the **Center Zone** are treated as one ordered group whose visual center aligns to the **Bar Surface Midpoint**.
- Oversized **Center Zone** content remains aligned to the **Bar Surface Midpoint** and may be clipped by the **Bar Surface** content bounds.
- **Zones** do not visually overlap; Start and End content gives way before it can draw over **Center Zone** content.
- Visible content from neighboring **Zones** is separated by the **Module Gap**.
- Clipped **Zone** content is not interactive outside its visible bounds.
- A **Center Zone** with no visible content reserves no space, creates no **Module Gap**, and does not prevent Start or End content from crossing the **Bar Surface Midpoint**.
- When the **Center Zone** has visible content, Start and End content are constrained independently against that visible **Center Zone** content.
- When the **Center Zone** has no visible content and Start and End content cannot both fit, Start remains anchored to Start, End remains anchored to End, and both give way at their collision boundary.
- Visual **Center Zone** priority does not change **Keyboard Focus** navigation order.
- Canopa Shell does not provide an alternate **Center Zone** mode that centers content in the remaining space between Start and End content.

### Modules

- A **Bar** contains zero or more **Modules**.
- A **Bar** with no **Modules** is valid and may still produce **Bar Surfaces**.
- A **Module** may be placed in multiple **Bars**.
- Placing the same **Module** more than once within one **Bar** is a **Configuration Error**.
- A **Module** is configured globally by name; a **Bar** places **Modules** but does not redefine their internals.
- A **Module** reused across **Bars** shares runtime state by name when its data source is global.
- Separately named **Modules** of the same kind do not share runtime state.
- A synthesized built-in **Module** uses its **Module Kind** as its **Module Name**.
- **Module Names** are globally unique, exact, and case-sensitive.
- Duplicate **Module Names** are **Configuration Errors**.
- All built-in **Module Kinds** reserve their default **Module Names**, whether or not they can be synthesized.
- A declared **Module** may use a reserved built-in **Module Name** only when its **Module Kind** matches that reserved name.
- A declared **Module** may override **Global Defaults** for its matching built-in **Module Kind**.
- A declared **Module** may not change the **Module Kind** associated with a reserved built-in **Module Name**.
- A placement name resolves first to a declared **Module Name**, then to a built-in **Module Kind** with meaningful, safe **Global Defaults**.
- A placement name that resolves to neither a declared **Module Name** nor a synthesizable built-in **Module Kind** is a **Configuration Error**.
- Referencing a built-in **Module** with meaningful, safe **Global Defaults** is valid even when that **Module** is not explicitly declared.
- **Modules** that require user-provided commands or labels must be explicitly configured.
- A **Module** may render differently on different **Bar Surfaces** when its information is output-sensitive.
- A **Module** may expose zero or more **Module Actions**.
- A failed **Module Action** does not change the normal **Module** display by default.
- Output-sensitive **Module Actions** use the originating **Bar Surface** context.
- A **Module Action** may run a **Panel Action** when configured.
- A **Module** may expose a **Tooltip**.
- A **Module** may expose a **Menu**.
- An **Unavailable Module** normally renders no visible content, leaves no **Module Gap**, has no **Tooltip**, and is skipped by **Keyboard Focus** navigation. The **Notification Module** deliberately stays visible, with an unavailable indicator, tooltip and keyboard actions while it reconnects (Canopa ADR 0004).
- An **Unavailable Module** can recover automatically when its data source or **Compositor Integration** becomes available.
- An **Empty Module** renders no visible content, leaves no **Module Gap**, has no **Tooltip**, and is skipped by **Keyboard Focus** navigation.
- An **Empty Module** is not a runtime failure.
- A **Bar Surface** reflows visible targets when a **Module** becomes unavailable, empty, visible, or recovered.

### Media

- Every **Media Module** observes the same selected player.
- The **Media Panel Provider** changes that shared selection and exposes only supported controls.
- Playback controls target the captured player owner and track; they never substitute another player.
- Existing players own media playback; an empty player list produces an **Empty Module**.
- Media **Device Keys** control the same selected player, or the automatic choice without a **Media Module**.

### Device keys

- One **Indicator** is visible at a time; a **Device Key** press replaces it and restarts its timeout.
- An **Outside Change** never shows at startup, and a press's own echo never shows twice.
- Services own the levels: audio through the Pulse-protocol server, brightness through logind, media through MPRIS.

### Network

- A **Network Status Integration** reports connectivity and the active connection label.
- A **Wi-Fi Control Integration** performs **Wi-Fi Scans** and **Wi-Fi Connection Attempts** from an **External Panel Provider**.
- A **Network Status Integration** does not own **Wi-Fi Scans**, **Wi-Fi Connection Attempts**, or **External Panel Content**.
- When NetworkManager is available, the network status provider and **Wi-Fi Control Integration** use NetworkManager.
- When NetworkManager is unavailable, systemd-networkd may provide **Network Status Integration** and iwd may provide **Wi-Fi Control Integration**.
- When no **Wi-Fi Control Integration** is available, the network **Module** remains status-only.
- **Network Status Integration** distinguishes unknown connectivity, captive portals and confirmed Internet access. networkd's online state means connected links, not measured Internet access; its label lists usable online links and refreshes on link changes.
- Battery readings retain UPower presence and power state. An absent display battery maps to **Empty Module**, while charging, full and pending states remain distinct for formatting.
- Audio observation and adjustment use the same PulseAudio protocol server, including PipeWire's compatibility server. Losing the default sink invalidates its reading without requiring the audio server to disconnect.
- The volume **Module** opens the **Audio Panel Provider** by default. Existing
  module `open-panel` bindings resolve to Sound; other explicit actions take
  precedence. The provider uses existing v5 rich controls, preserves channel
  proportions when adjusting volume, excludes monitor sources, and binds changes
  to an exact live **Audio Device** ([ADR 0016](adr/0016-audio-device-panel-provider.md)).
- Canopa Shell does not mix NetworkManager status with iwd Wi-Fi control.

### Dynamic Modules and Data Sources

Configured names and child identities are structurally distinct. A child identity contains its configured parent and typed runtime key; formatting a name never creates a child identity. Parent ownership is stored once in the identity. See [ADR 0019](adr/0019-structural-module-identities.md).

- A **Dynamic Module** owns the placement and configuration for its runtime child **Module Instances**.
- Tray and **Niri Integration** pass **Dynamic Module Child Facts** to the **Dynamic Module Lifecycle**; they do not construct child **Module Instances**.
- The **Dynamic Module Lifecycle** is explicit about supported **Dynamic Module Families**.
- The **Dynamic Module Lifecycle** owns **Dynamic Module Child Key** matching, child construction, child reconciliation, stale focus and hover cleanup, inherited actions and interactions, and layout expansion for **Dynamic Modules**.
- The **Dynamic Module Lifecycle** returns a **Dynamic Module Reconciliation Plan** rather than directly mutating **Module Runtime**.
- Reconciliation in the **Dynamic Module Lifecycle** consumes a **Dynamic Module Runtime Snapshot**. Placement expansion reads Module Store’s configured-name and ordered-child indexes.
- Durable child **Module Instance** state remains in **Module Runtime**.
- The **Dynamic Module Lifecycle** supports per-family child update policy: some families update matching child **Module Instances** in place, while others rebuild matching child **Module Instances** from current child source facts.
- The **Dynamic Module Lifecycle** owns per-family child ordering policy. Workspace child **Module Instances** are ordered by compositor workspace index; tray child **Module Instances** are ordered deterministically by SNI runtime identity.
- Child **Module Instances** produced by a **Dynamic Module** are individually navigable and actionable when they represent distinct user targets.
- **Dynamic Modules** expand in **Panel** module layouts the same way they expand in **Bar** module layouts.
- A **Module Instance** may depend on a **Module Data Source**.
- Multiple **Module Instances** may share one **Module Data Source Identity**.
- A **Module Data Source** emits **Module Data Source State**, not render-specific module state.
- **Module Data Source Runtime** maps active **Module Runtime** source requirements into subscriptions and owns first emission, recovery, and **Unavailable Module** transitions for **Module Data Sources**.
- **Module Runtime** maps **Module Data Source State** into per-**Module Instance** render state.
- Pending first emission is internal **Module Data Source Runtime** state, not a public render state.
- When a **Module Data Source** recovers and emits valid **Module Data Source State**, **Module Runtime** automatically remaps all dependent **Module Instances**.
- Available **Module Data Source State** maps into either visible render state or an explicit **Empty Module** outcome.
- An **Empty Module** outcome is module-owned; runtime lifecycle does not infer it from blank rendered content.

### Tooltips, Menus, and Foreground

- The visible **Module** or child **Module Instance** target owns the **Tooltip** shown for that target.
- A **Tooltip** closes when its originating visible target disappears.
- A **Tooltip** closes when a **Panel** opens.
- A **Tooltip** is not **Foreground**.
- **Tooltips** and **Panels** respect the compositor's reserved bar space, including **Bar Surfaces** owned by other processes; their layer-shell margins are relative to the usable area.
- A **Bar Surface** shows at most one **Tooltip** at a time.
- Opening a **Menu** closes open **Tooltips** on the same **Bar Surface**.
- Canopa Shell shows at most one **Menu** at a time.
- A **Menu** closes when its originating visible target disappears.
- **Keyboard Focus** enters a **Menu** while it is open and returns to the originating target when the **Menu** closes, if that target still exists.
- A **SNI Menu** is a **Menu**.
- **SNI Menus** do not show **Menu Search** by default.
- A user may configure **Menu Search** for **SNI Menus**.
- A **SNI Menu** with configured **Menu Search** renders a visible search input while the **Menu** is open.
- When a **SNI Menu** layout updates while open, Canopa Shell resets that **Menu** to its top level with empty **Menu Search** text and initial focus.
- **Menus** and **Tooltips** do not inherit the **Layer** of their **Bar Surface**.
- Canopa Shell has at most one **Foreground** at a time.
- Opening a **Panel** or **Menu** replaces the current **Foreground**.
- The first click outside a **Panel** or **Menu** dismisses it without activating
  the underlying application or **Bar**. **Foreground** owns the **Panel**'s
  transparent dismissal surfaces; see [ADR 0015](adr/0015-dismiss-foreground-on-outside-click.md).
- Opening a **Panel** closes any open **Menu**.
- Opening a **Menu** closes any open **Panel**.
- Every request that opens, toggles, or closes a **Panel** or **Menu** goes through **Foreground**.
- **Foreground** owns stale close handling for physical **Panel** and **Menu** overlay windows.
- **Foreground** is the source of truth for interactive overlay role classification.
- **Foreground** communicates side-effect work through effects; app-owned side effects remain in the app adapter.

### Panels

- Canopa Shell shows at most one **Panel** at a time.
- **Foreground** owns **Panel** open, toggle, replacement, close, and physical overlay window identity.
- **Panel Runtime** is internal to **Foreground**; callers use **Foreground** interfaces rather than talking to **Panel Runtime** directly.
- A **Panel** becomes logically active when **Foreground** accepts the open request and prepares **Panel Runtime** content state, before its overlay window is created.
- Replacing a **Panel** makes the old **Panel** logically gone immediately, even while its old overlay window is still closing.
- An external **Panel** window close clears **Panel Runtime** state only through **Foreground** and only when **Foreground** identifies that window as the current **Panel**.
- A **Panel** belongs to exactly one originating **Bar Surface**.
- A **Panel** has exactly one **Panel Origin**.
- A **Panel Origin** may be the **Bar Surface** itself or a **Module** on that **Bar Surface**.
- A **Panel** does not change the size, **Edge Span**, or normal module rendering of its originating **Bar Surface**.
- A **Panel** renders as an overlay and does not reserve exclusive output space.
- A **Panel** opens beside the **Edge** of its originating **Bar Surface**.
- A module-originated **Panel** with automatic presentation aligns to the originating **Module** when its bounds are available and falls back to the **Bar Surface** otherwise.
- A **Bar** may select drawer presentation for its **Panels**. On a vertical **Bar**, ordinary content uses a consistent width and fills the usable vertical edge unless configured dimensions or caps override it. The Notification Center prefers a wider reading area. A capped drawer stays at the start of the usable edge, independently of its **Panel Origin**. Horizontal **Bars** retain automatic presentation.
- On a vertical **Bar**, adaptive presentation uses a drawer for **Bar Panel**, **Panel Menu**, and external provider content. **Module Detail Content** uses an automatic popover anchored to its originating **Module**.
- A drawer shows the active **Panel** content, with a title and an explicit close control. **Modules** on the **Bar Surface** select content; a drawer does not add a second section-navigation model.
- Drawer and automatic **Panel** presentation share title/close chrome, body spacing, menu rows and focus treatment. Content-preferred sizes include the common chrome; explicit dimensions and caps remain authoritative.
- **Foreground** may retain one inert outgoing **Panel** presentation for a bounded exit animation after logical interaction ends. It cannot receive input, provider updates or accessibility actions. Replacement or origin removal discards it, and its completion cannot clear a newer **Panel**.
- Per-**Bar** opening and closing durations control directional slide/fade presentation. Zero disables the corresponding transition. Transition frames never change exclusive space or surface placement; idle **Panels** request no animation frames. See [ADR 0014](adr/0014-share-panel-chrome-and-bound-transition-lifetime.md) for capture limits and transition lifetime.
- The **Panel Origin** remains visually marked while the **Panel** is open.
- A **Panel** closes when Escape or an explicit close action closes it, or when its originating **Bar Surface** disappears.
- A **Panel** may remain open when focus moves from the originating **Bar Surface** into the **Panel**.
- Escape closes a **Panel** unless the active **Panel** content has a more specific first-Escape behavior.
- Omitting an exact configured **Panel** dimension lets **Panel Sizing** choose that dimension from content, **Edge Span** fill behavior, or fallback defaults.
- Automatic **Panel Sizing** keeps **Module Detail Content** compact with content-preferred sizes. Provider popovers prefer a stable 600-logical-pixel height for lists and forms, bounded by a known vertical bar span and configured height limits. The `notifications` provider instead prefers 480 logical pixels of width and fills the usable screen height on every edge. Its loading, reading and settings views share the same geometry; explicit dimensions and caps still apply.
- **Panel Sizing** uses **Edge Span** fill sizing for **Panel Menu** content and a **Bar**'s configured panel module layout.
- **Panel Sizing** treats explicit configured dimensions as overrides and configured maximum dimensions as caps.
- **Panel Sizing** produces a **Panel Physical Plan** before the application adapter opens the physical **Panel** overlay window.
- A **Panel** with an explicit configured dimension larger than the corresponding configured maximum dimension is a **Configuration Error**.
- A **Module** may provide default **Module Detail Content** for its own module-originated **Panel**.
- A **Module** that provides **Module Detail Content** may provide a content-preferred size for that detail content.
- A **Module Detail Session** belongs to the **Module Detail Content** of one open **Panel**.
- A **Module Detail Session** is module-owned; generic **Panel Runtime** does not interpret module-specific detail events or detail state.
- A **Module Detail Session** resets each time module-originated **Module Detail Content** opens.
- A **Module Detail Session** may request app-owned effects for shared app infrastructure rather than reimplementing those effects inside the **Module**.
- **External Panel Content** may use **Selectable Search** to search **Wi-Fi Networks** without modeling those networks as **Panel Menu** entries.
- An **External Panel Provider** may be launched on demand when Canopa Shell opens the **Panel** that will host its **External Panel Content**.
- An **External Panel Session** ends when its hosting **Panel** closes unless a later provider lifecycle explicitly keeps the provider alive.
- A launched **External Panel Provider** communicates over standard input and standard output using its explicitly configured protocol. The default `niribar-v1` uses framed JSON; `canopa-v4` uses bounded JSON lines. Both wire models and codecs live in `canopa-panel-protocol`; prepared images, focus and list search text remain host state ([Canopa ADR 0006](../adr/0006-shared-provider-contracts.md)) ([ADR 0011](adr/0011-explicit-canopa-panel-protocol-adapter.md)).
- Built-in Wi-Fi, Sound and Notification Center providers use `canopa-v5`: provider-first JSON lines, complete form or rich content, interaction revisions and explicit close. Compatible input echoes preserve the form revision; new operations retire it ([Canopa ADR 0007](../adr/0007-unified-panel-protocol.md)). User-declared providers retain explicit version selection and the v1 default.
- A framed-v1 **External Panel Session** starts with a host-first protocol-version handshake. A Canopa-v4 session starts with a provider-first, versioned complete content replacement.
- An **External Panel Provider** updates **External Panel Content** by sending a complete replacement content model, not incremental patches.
- Replacing **External Panel Content** redraws the existing **Panel** window. Updates and local text/search edits preserve that window's focus and scroll state; a control with a different identity starts with fresh input state.
- A failed **External Panel Provider** renders provider error content in its hosting **Panel** instead of silently closing the **Panel**.
- **External Panel Session** owns stale provider output filtering and provider failure conversion for its active **Panel**.
- **External Panel Session** owns the current **External Panel Content** document for its active **Panel**.
- **External Panel Session** owns local echo for **External Selectable List** search text and **External Text Input** values before provider responses arrive.
- Closing an **External Panel Session** is idempotent; the first close ends the provider interaction, and repeated close requests do not emit additional provider close work.
- **Foreground** owns whether an **External Panel Session** still has an active provider-backed **Panel** to update.
- **External Panel Content** cannot ask Canopa Shell to run arbitrary **Configured Commands**.
- User interactions with **External Panel Content** are sent to the **External Panel Provider** as typed events addressed by **External Panel Element IDs**.
- Canopa Shell does not interpret **External Panel Element IDs** as domain meaning.
- Rich **External Panel Content** may attribute an item to a source label and local/theme icon, and decorate section titles with the same icon type. The host prepares bounded icon assets off the UI thread, renders a decorative icon or initial badge, and exposes the label to native accessibility. Duplicate section headings may be omitted when all items already name that source ([Canopa ADR 0010](../adr/0010-rich-item-source-attribution.md)).
- Rich **External Panel Content** may include an **External Slider**. Replacements cancel unfinished gestures; committed values must match the published range/grid and current generation/revision ([Canopa ADR 0012](../adr/0012-rich-panel-sliders.md)).
- **External Panel Content** may include **External Selectable Lists**.
- Canopa Shell owns **Selectable Search** state and matching for **External Selectable Lists** unless a later provider-filtered list mode is introduced.
- The first **External Panel Content** model supports only a small set of primitives needed for list selection, text entry, status, and simple actions.
- The first external-provider platform supports **External Panel Content** only, not provider-owned **Menus** or **Tooltips**.
- Canopa Shell has at most one active **External Panel Session** at a time.
- **Configured Panel Providers** are globally named by **Panel Provider Name** and may be referenced by **Modules** or **Panel Actions**.
- **Panel Provider Resolution** prefers a user-declared **Configured Panel Provider** over a built-in provider with the same **Panel Provider Name**.
- **External Panel Session** consumes **Panel Provider Resolution** results rather than reading raw provider configuration directly.
- A built-in **Module** may provide a default **Configured Panel Provider**.
- User configuration may override or disable a built-in **Module**'s default **Configured Panel Provider**.
- A configured **Module Action** takes precedence over a built-in **Module**'s default provider activation.
- A missing optional built-in **Configured Panel Provider** does not make its **Module** unavailable.
- A missing explicitly configured **Configured Panel Provider** is surfaced as a provider failure when the user tries to open it.
- The provider platform should exist before domain-specific providers such as Wi-Fi or audio providers are integrated.
- Clock **Module Detail Content** shows the current local month by default and may let the user browse other months while that **Panel** remains open.
- Clock month browsing resets to the current local month each time clock **Module Detail Content** opens.
- Clock month browsing is available through pointer controls and keyboard controls while clock **Module Detail Content** is focused.
- Clock month browsing provides a way to return directly to the current local month.
- Clock **Module Detail Content** marks today when today is visible and does not select individual days.
- Clock **Module Detail Content** displays calendar weeks starting on Monday.
- Clock **Module Detail Content** leaves calendar cells outside the displayed month blank.
- Clock calendar detail behavior is not user-configurable unless a later requirement introduces a real user choice.
- A **Bar** may configure a default **Panel** module layout using **Start**, **Center**, and **End** groups.
- A **Bar**'s default **Panel** layout is independent from the **Bar**'s normal module layout.
- A **Bar**'s default **Panel** layout renders as ordered sections, not as stretched bar zones.
- A **Panel** module layout may reference **Modules** that are not present in the **Bar**'s normal module layout.
- **Bar Panel** Modules may render compact controls separately from full **Module Detail Content**. Their input is checked against the active Panel window and layout, retaining its Bar Surface and anchor. Scoped detail actions select the activated Module; provider navigation keeps the originating Panel's origin.
- Wi-Fi and Sound controls remain reachable in a **Bar Panel** when status is unavailable. Full connection and device controls remain owned by their **External Panel Providers**.
- Opening an **External Panel Provider** from a **Bar Panel** with the same origin offers Back to that Bar Panel. Provider updates retain this return; dismissal and unrelated replacement discard it.
- **Bar Panel** card focus follows its visible actionable Modules; activation uses each Module's compact-control interaction (DND uses the notifications secondary action).
- Trailing Button Modules in a **Bar Panel** form a fixed two-column footer. Their ordering and activation follow the same **Module** layout and actions; the bundled session controls use configured confirmation **Panel Menus** with Cancel first.
- Placing the same **Module** more than once within one **Panel** module layout is a **Configuration Error**.

### Panel Actions and Panel Menus

- A **Launcher** is a `launcher` **Panel Menu Source** combining applications,
  file/folder names, open windows, desktop controls, arithmetic and unit conversions,
  and explicitly configured
  executable providers, including bundled web/date tools. Searchable actions, pins
  and aliases remain child modes/preferences of this same Panel Menu. See
  [shared ADR 0131](../adr/0131-launcher-preferences-and-bundled-extensions.md).
  It uses existing **Panel** placement and **Foreground** lifetime. Its bounded
  asynchronous groups retain selection by result identity and reject stale
  query/results/action revisions. Existing **Selectable Search** contracts for
  other sources keep their meaning. See [ADR 0023](adr/0023-unified-launcher-providers.md)
  and the [launcher guide](launcher.md) for provider ownership and configuration.

- **Panel Actions** are typed operations, not shell commands by default.
- A **Panel Action** trigger may be a pointer trigger such as `click` or `right-click`, or a focused keybind such as `Ctrl+E`.
- Repeating a **Module** click that targets its current **Panel** on the same **Bar Surface** dismisses that **Panel**. A repeat click also cancels that origin's pending **External Panel Session**. Direct **Panel Actions** keep their explicit open/toggle semantics.
- **Module** keybind **Panel Actions** fire only while that **Module** is the **Keyboard Focus** target.
- **Bar** keybind **Panel Actions** fire while that **Bar Surface** or its **Panel** has keyboard focus.
- If both a focused **Module** and its **Bar** define the same keybind, the **Module** binding takes precedence.
- A **Bar Surface Interaction** may trigger a **Panel Action** when configured by the **Bar**.
- A **Bar Surface Interaction** on empty bar background is distinct from interacting with a **Module**.
- A **Panel Action** may open an **External Panel Session** through a **Configured Panel Provider**.
- A **Panel Menu** is global named configuration and can be opened from any **Panel Action**.
- A **Panel Menu** has exactly one **Panel Menu Source**.
- A **Panel Menu Source** may be declared by explicit `item` configuration or by a `source` declaration.
- A **Panel Menu** cannot mix explicit `item` configuration with a dynamic `source` declaration.
- A dynamic **Panel Menu Source** supplies current **Panel Menu** entries at runtime without rewriting user configuration.
- A dynamic **Panel Menu Source** with no current entries is a valid empty **Panel Menu**, not a **Configuration Error**.
- A dynamic **Panel Menu Source** is refreshed when its **Panel Menu** opens and remains stable while that **Panel** is open.
- A **Panel Menu** item runs exactly one **Panel Action**.
- A **Panel Menu** item may declare at most one **Panel Menu Entry Icon**.
- A static **Panel Menu Entry Icon** with an empty or path-like name is a **Configuration Error**.
- A **Panel Menu** controls whether **Panel Menu Entry Icons** are rendered for its entries.
- A **Panel Menu** does not render **Panel Menu Entry Icons** unless icon rendering is explicitly enabled.
- A **Panel Menu** applies icon rendering consistently across its menu levels and search results.
- A **Panel Menu** that renders **Panel Menu Entry Icons** preserves row label alignment when an entry has no icon or its icon cannot be resolved.
- **Panel Menu Entry Icons** are prepared off the UI thread; pending icons preserve the same alignment, and scrolling never waits for icon preparation ([ADR 0013](adr/0013-prepare-panel-menu-icons-off-the-ui-thread.md)).
- Dynamic **Panel Menu Sources** may supply **Panel Menu Entry Icons** from their own entry metadata.
- **Panel Menu Prompt** is menu metadata, not a selectable menu entry.
- **Panel Menu Prompt** may appear as a search input placeholder, panel header, or both.
- **Panel Menu Prompt** rendering is a general **Panel Menu** capability, even when the first public caller is **Dmenu Selection Mode**.
- An installed-applications **Panel Menu Source** includes **Installed Desktop Applications**, not hidden, terminal-only, or system desktop entries.
- Desktop-environment-specific application metadata is honored when deciding whether an **Installed Desktop Application** belongs in the current environment.
- Activating an **Installed Desktop Application** from a **Panel Menu** launches the application and closes the **Panel**.
- Launching an **Installed Desktop Application** from a **Panel Menu** is source-owned behavior, not a separate public **Panel Action** type.
- Launching **Installed Desktop Applications** follows Canopa Shell's centralized process-launch boundary.
- An application `launch` **Panel Action** and **Installed Desktop Application** activation hand ownership to the desktop session, independently of Canopa Shell's service and command timeout. niri Spawn is preferred; a systemd user scope is the fallback when no niri socket is advertised ([ADR 0012](adr/0012-session-owned-application-launches.md)).
- `launch` accepts literal argv strings. **Configured Commands** keep their bounded automation lifetime; the application lifetime is not inferred from a command's executable name.
- Installed entries retain their selected source path and localized metadata. GIO owns ordinary desktop-entry field expansion and working directory. D-Bus-enabled entries use an acknowledged session-bus Activate call with no retry after an ambiguous failure. Synchronous launch failures are logged; later application failures are reported by the session launcher.
- A failed launch of an **Installed Desktop Application** is a failed **Panel Action** and does not change the normal **Panel Menu** entry by default.
- A **Desktop Application ID** identifies an **Installed Desktop Application** even when multiple applications share the same display name.
- A **Panel Menu** may use secondary metadata to disambiguate **Installed Desktop Applications** with the same display name.
- **Installed Desktop Applications** may supply best-effort **Panel Menu Entry Icons**, but missing icons do not prevent display or launch.
- Invalid **Panel Menu Entry Icons** from installed application metadata are treated as missing icons, not **Configuration Errors**.
- An installed-applications **Panel Menu Source** presents **Installed Desktop Applications** as a flat list by default.
- **Menu Search** uses **Selectable Search** for configured **Panel Menus**, installed-applications **Panel Menu Sources**, **Dmenu Panel Menu Sources**, and configured searchable **SNI Menus**.
- **Selectable Search** uses the same matching, ranking, and nested-entry behavior everywhere it is enabled.
- **Selectable Search** uses fuzzy matching as the default behavior rather than offering a substring-search mode.
- **Selectable Search** fuzzy matching requires query characters to appear in order.
- **Selectable Search** is ASCII case-insensitive.
- **Selectable Search** splits search text on whitespace and requires every query token to match.
- **Selectable Search** ranks matching entries by match quality and uses source order as the tie-breaker.
- **Selectable Search** ranks substring matches ahead of non-contiguous fuzzy matches when other priority rules are equal.
- **Selectable Search** preserves source order when the search text has no non-whitespace tokens.
- **Selectable Search** returns all matching entries rather than applying a hidden result cap.
- **Selectable Search** ranks visible leaf labels ahead of secondary metadata, and secondary metadata ahead of breadcrumb-only ancestor matches.
- **Selectable Search** can match breadcrumb ancestor labels for nested entries as lowest-priority text.
- **Selectable Search** uses Canopa Shell-owned matching behavior rather than exposing a third-party matcher as domain behavior.
- **Menu Search** for **Installed Desktop Applications** matches application names, descriptions, generic names, and keywords.
- A selectable menu may allow a **Custom Menu Value** when its source or configuration enables typed values.
- A **Custom Menu Value** appears after fuzzy-matched entries, or first when there are no matching entries.
- Activating a **Selectable Menu Session** produces a generic selection result; the owning runtime interprets that result as a **Panel Action**, **Installed Desktop Application** launch, **Dmenu Selection Request** response, or **SNI Menu** item activation.
- A **Selectable Menu Session** owns both pointer hover and keyboard focus for its entries.
- Canopa Shell has at most one active **Selectable Menu Session** at a time.
- A selectable menu may contain nested items, but Canopa Shell displays one active menu level at a time unless **Menu Search** is active.
- **Menu Search** matches nested items and displays matching results with breadcrumb context.
- A searched **Panel Menu** entry keeps its own **Panel Menu Entry Icon**; breadcrumb ancestors do not provide the result icon.
- Activating a command item in a **Panel Menu** closes the **Panel** by default.
- Activating a command from module panel content does not close the **Panel** by default.

### Dmenu

- **Dmenu Selection Mode** is a standalone CLI lifecycle from the caller's perspective, but the running Canopa Shell instance owns the visible selector surface.
- **Dmenu Selection Mode** sends a **Dmenu Selection Request** to the running Canopa Shell instance.
- **Dmenu Selection Mode** fails without opening a fallback UI when no running Canopa Shell instance is available to handle the **Dmenu Selection Request**.
- **Dmenu Selection Mode** reads its standard input completely before sending one bounded **Dmenu Selection Request**.
- **Dmenu Selection Mode** aims for dmenu-style selection behavior, not full dmenu flag compatibility.
- **Dmenu Selection Mode** accepts prompt text with `-p` or `--prompt`.
- **Dmenu Selection Mode** uses newline as the default input delimiter.
- **Dmenu Selection Mode** delimiter options interpret common escapes such as `\n`, `\t`, `\0`, and `\\`.
- **Dmenu Selection Mode** delimiter options apply only to plain **Dmenu Entry Streams**; combining a custom delimiter with rich JSONL input is a usage error.
- The **Dmenu Selection Mode** prompt becomes a **Panel Menu Prompt** for the request-owned **Panel Menu**.
- **Dmenu Selection Mode** treats the custom delimiter as input parsing only; successful output is always the selected value followed by a newline.
- Empty input is valid in **Dmenu Selection Mode**.
- **Dmenu Selection Mode** preserves duplicate input entries in display order.
- **Dmenu Selection Mode** preserves duplicate **Rich Dmenu Entries** in display order, regardless of shared labels or values.
- **Dmenu Selection Mode** allows the user to submit a **Custom Menu Value** by default.
- **Dmenu Selection Mode** can be explicitly restricted to provided entries with `--no-custom`.
- Empty input with `--no-custom` offers no selectable entries and can only be cancelled.
- When **Dmenu Selection Mode** has a focused matching entry, normal activation returns that focused entry's value.
- Submitting a **Custom Menu Value** uses a distinct custom-value affordance.
- **Dmenu Selection Mode** writes the selected value and does not run configured **Panel Actions**.
- **Dmenu Selection Mode** exits successfully only when the user selects or submits a value.
- Cancelling **Dmenu Selection Mode** exits distinctly from CLI usage errors and IPC/request failures.
- **Dmenu Selection Mode** has no default timeout while waiting for user selection.
- If the running Canopa Shell instance disconnects before completing a **Dmenu Selection Request**, the CLI treats it as an IPC/request failure.
- A **Dmenu Selection Request** opens on the focused **Output** when known, or on one deterministic **Bar Surface** when no focused **Output** is known.
- Opening a **Dmenu Selection Request** closes any existing **Panel** or **Menu**.
- A running **Dmenu Selection Request** rejects additional **Dmenu Selection Requests** until it completes.
- A **Dmenu Selection Request** cancels when its **Panel** loses focus, consistent with other **Panels**.
- Clearing the **Panel Runtime** for an active **Dmenu Selection Request** returns a cancelled response to the requesting CLI instead of silently dropping the responder.
- A **Dmenu Selection Request** is not a configured **Panel Action**.
- A **Dmenu Selection Request** is displayed as a **Panel Menu** with a **Dmenu Panel Menu Source**.
- A **Dmenu Panel Menu Source** returns its activated value to the requesting CLI process instead of running a configured **Panel Action**.
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

### Focus and Compositor Integration

- **Keyboard Focus** belongs to a **Bar Surface**.
- Physical window lifetime, geometry, focus and pointer events are always observed, including on empty or passive-only **Bar Surfaces**; optional module backend capabilities do not gate surface input. A physical bar close retires its surface, tooltip and foreground origin through the same ownership path as output removal.
- Keyboard target eligibility is an instance property derived from configured **Module Actions**, intrinsic interactions and resolved **Tooltip** availability. Visible content without any of these remains available to rendering and pointer hover but is skipped by keyboard navigation.
- **Keyboard Focus** navigation includes visible targets that are actionable or inspectable.
- A target with a hover **Tooltip** is inspectable and therefore participates in **Keyboard Focus** navigation.
- A target that exposes a **Menu** participates in **Keyboard Focus** navigation.
- A **Bar Surface** with no visible actionable or inspectable targets has no **Keyboard Focus** target.
- A **Global Panel Toggle** closes the currently open **Panel**, or opens the default **Panel** for the focused **Output** when no **Panel** is open.
- If no focused **Output** is known, a **Global Panel Toggle** opens one deterministic **Bar Surface** default **Panel**, not one **Panel** per **Bar Surface**.
- A **Global Panel Menu Open** uses one **Bar Surface** on the focused **Output**, with the same deterministic fallback as **Global Panel Toggle** when that output has no bar or focus is unknown.
- A **Global Panel Menu Open** uses exact, case-sensitive menu names, replaces **Foreground** through the normal **Panel** opening path, and opens rather than toggles on repeated requests.
- A **Global Panel Menu Open** acknowledges acceptance without waiting for selection. Unknown menu names and missing **Bar Surfaces** reject the request without replacing **Foreground**.
- A **Global Panel Menu Toggle** matches the exact menu name regardless of its **Bar Surface** or **Module** origin. Cancelling a pending opening leaves a different active **Panel Menu** intact and ignores late application-discovery results.
- A missing **Compositor Integration** makes only the dependent **Modules** unavailable; it does not prevent **Bar Surface** creation.
- Compositor-backed **Modules** use generic names for portable user-facing concepts and compositor-specific names only for non-portable concepts.
- Workspace-capable **Modules** render **Workspaces** relevant to the **Bar Surface**'s **Output** by default.
- A focused-window **Module** shows the **Focused Window** on the focused **Output** and the **Active Window** for non-focused **Outputs**.
- Niri is the first **Compositor Integration**, but Canopa Shell's **Bar Surface** model is not limited to Niri IPC.

### Configuration Errors

- A **Configuration Error** is reported actionably and prevents the invalid configuration from being used.
- Saved configuration and explicit reload requests use the same complete candidate validation and installation path; failures preserve the running configuration and **Foreground**. Successful effective changes retire **Foreground**, reconcile **Bar Surfaces**, and preserve unchanged module subscriptions and shared readings. See [ADR 0021](adr/0021-live-configuration-reload.md).
- A parsed **Config** contains only **Configured Commands** accepted by **Command Policy**.
- Raw shell or argv command syntax is internal to **Configured Command** parsing and is not exposed by the runtime model.
- A parsed **Configured Command** carries the execution timeout resolved from **Command Policy**.
- **Command Policy** is not a runtime execution input after parsing; runtime code executes policy-resolved **Configured Commands**.
- Configuration owns **Configured Command** validation; command execution code owns the process-launch adapter for policy-resolved **Configured Commands**.
- A shell-form **Configured Command** is a **Configuration Error** when **Command Policy** disables shell-form commands.
- An unknown **Module** reference in configuration is a **Configuration Error**.
- A valid **Module** with a missing runtime dependency becomes an **Unavailable Module**, not a **Configuration Error**.

### Module presentation

- Ordinary **Modules** provide presentation values and optional fallback/decorations; the shared renderer owns format layout, style defaults and edge-specific rotation (ADR-0020).
- **Network Status Integration** and shipped **Wi-Fi Control Integration** consume the same live daemon-bundle policy, and retire state tied to a replaced owner (ADR-0009).

## Resolved Ambiguities

- **Bar** is configured identity; **Bar Surface** is the live Wayland surface produced from it.
- **Module** is configured content; **Module Instance** is runtime state and behavior.
- **Start**, **Center**, and **End** are logical **Zones**. Physical `top` and `bottom` names are **Edge** values, not **Zone** names.
