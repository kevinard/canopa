# Niribar

Niribar is a status bar application for the Niri Wayland compositor. It renders configurable bar surfaces on Wayland outputs and fills them with modules that show information or trigger actions.

## Language

**Bar**:
A named configuration that defines layout, output targeting, size, expansion behavior, style, and module placement.
_Avoid_: Surface, window

**Bar Name**:
The user-facing identifier for a **Bar**.
_Avoid_: Surface name, output name

**Bar Surface**:
A live Wayland layer-shell surface created for a specific **Bar** on a specific **Output**.
_Avoid_: Bar, window

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

**Module Instance**:
The runtime state and behavior of a **Module** while Niribar is running.
_Avoid_: Module, widget instance

**Dynamic Module**:
A **Module** that produces runtime child **Module Instances** from external state.
_Avoid_: Generated module, synthetic module

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
_Avoid_: Event, subscription, refresh

**Tooltip**:
Supplemental information shown for a **Module** on a **Bar Surface**.
_Avoid_: Popup action, module display

**Menu**:
An interactive popup exposed by a **Module** or child **Module Instance**.
_Avoid_: Tooltip, action

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

**Expansion**:
The user-visible state where a **Bar Surface** presents its expanded layout instead of its collapsed layout.
_Avoid_: Open, active

**Expansion Trigger**:
The configured **Bar Surface Interaction** that changes **Expansion**: click or hover.
_Avoid_: Global toggle, keybind

**Bar Surface Interaction**:
A user interaction with a **Bar Surface** that is not targeted at a **Module**.
_Avoid_: Module action

**Global Toggle**:
An IPC-triggered request to change **Expansion** without direct **Bar Surface** interaction context.
_Avoid_: Toggle all, keybind toggle

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
- A **Bar Surface** belongs to exactly one **Bar** and exactly one **Output**.
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
- A **Module Action** may change **Expansion**.
- A **Module** may expose a **Tooltip**.
- A **Module** may expose a **Menu**.
- A **Dynamic Module** owns the placement and configuration for its runtime child **Module Instances**.
- Child **Module Instances** produced by a **Dynamic Module** are individually navigable and actionable when they represent distinct user targets.
- The visible **Module** or child **Module Instance** target owns the **Tooltip** shown for that target.
- A **Tooltip** closes when its originating visible target disappears.
- A **Tooltip** closes when its owning **Bar Surface** changes **Expansion** state.
- A **Bar Surface** shows at most one **Tooltip** at a time.
- Opening a **Menu** closes open **Tooltips** on the same **Bar Surface**.
- Niribar shows at most one **Menu** at a time.
- A **Menu** closes when its originating visible target disappears.
- A **Menu** closes when its owning **Bar Surface** changes **Expansion** state.
- An open **Menu** pins passive **Expansion** changes for its owning **Bar Surface** until the **Menu** closes.
- An explicit **Global Toggle** or **Module Action** closes any affected open **Menu** before changing **Expansion**.
- **Keyboard Focus** enters a **Menu** while it is open and returns to the originating target when the **Menu** closes, if that target still exists.
- **Menus** and **Tooltips** do not inherit the **Layer** of their **Bar Surface**.
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
- **Expansion** capability is configured by the **Bar**, but current **Expansion** state belongs to each **Bar Surface**.
- A **Bar** may configure click or hover as its **Expansion Trigger**.
- A **Global Toggle** targets **Bar Surfaces** on the currently focused **Output** when known, and falls back to all **Bar Surfaces** when the focused **Output** is unknown.
- A **Global Toggle** ignores **Bar Surfaces** whose **Bar** has **Expansion** disabled.
- **Expansion** changes **Bar Surface** thickness, not **Edge Span**.
- A **Bar Surface Interaction** may trigger **Expansion** when configured by the **Bar**.

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
> **Dev:** "If the user expands the **Bar Surface** on `DP-1`, should the matching surface on `eDP-1` expand too?"
> **Domain expert:** "No — **Expansion** state belongs to the individual **Bar Surface**."
>
> **Dev:** "Is clicking the bar background a **Module Action**?"
> **Domain expert:** "No — it is a **Bar Surface Interaction** and may trigger **Expansion** when configured."
>
> **Dev:** "Is `niribar --toggle-expand` an **Expansion Trigger**?"
> **Domain expert:** "No — an **Expansion Trigger** is the configured direct **Bar Surface Interaction**; IPC uses **Global Toggle**."
>
> **Dev:** "What should `niribar --toggle-expand` affect?"
> **Domain expert:** "It should affect **Bar Surfaces** on the focused **Output** when known, and all **Bar Surfaces** only when no focused **Output** is known."
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
> **Dev:** "Does every named **Bar** need to repeat style and expansion settings?"
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
