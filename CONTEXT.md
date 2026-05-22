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
_Avoid_: Event, subscription, refresh, panel action when the action is not module-scoped

**Tooltip**:
Supplemental information shown for a **Module** on a **Bar Surface**.
_Avoid_: Popup action, module display

**Menu**:
An interactive popup exposed by a **Module** or child **Module Instance**.
_Avoid_: Tooltip, action, launcher

**Panel**:
A transient overlay surface opened from a **Bar Surface**. A **Panel** is anchored to its originating **Bar Surface** or **Module** and can render module detail content, a **Bar**'s configured panel module layout, or named **Panel Menu** content.
_Avoid_: Popup when referring to Niribar's generic panel

**Panel Origin**:
The **Bar Surface** or **Module** that triggered the current **Panel**.
_Avoid_: Active module when referring specifically to panel source

**Panel Action**:
A typed configured operation triggered by pointer input, focused keyboard input, **Panel Menu** item activation, or IPC.
Examples: `open-panel`, `toggle-panel`, `close-panel`, `open-menu`, `toggle-menu`, `command`.
_Avoid_: Shell action when the action is not a command

**Panel Menu**:
An app-owned, named launcher/menu list rendered inside a **Panel**.
_Avoid_: Menu when referring to SNI or other module-owned context menus

**Panel Menu Entry Icon**:
An optional XDG icon-theme name attached to a **Panel Menu** entry.
_Avoid_: Installed app icon when referring to the general menu entry capability

**Panel Menu Source**:
The configured origin of **Panel Menu** entries, either explicit user configuration or a dynamic runtime source such as installed desktop applications.
_Avoid_: Generated config, launcher backend

**Panel Menu Prompt**:
Optional text that gives user-facing context for a **Panel Menu** search or selection.
_Avoid_: Menu item, title when the text is not just a heading

**Panel Menu Search**:
The user-facing matching behavior that filters or ranks **Panel Menu** entries from typed input.
_Avoid_: Filter when referring to ranked matching behavior

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
- A **Module Action** may run a **Panel Action** when configured.
- A **Module** may expose a **Tooltip**.
- A **Module** may expose a **Menu**.
- A **Dynamic Module** owns the placement and configuration for its runtime child **Module Instances**.
- Child **Module Instances** produced by a **Dynamic Module** are individually navigable and actionable when they represent distinct user targets.
- The visible **Module** or child **Module Instance** target owns the **Tooltip** shown for that target.
- A **Tooltip** closes when its originating visible target disappears.
- A **Tooltip** closes when a **Panel** opens.
- A **Bar Surface** shows at most one **Tooltip** at a time.
- Opening a **Menu** closes open **Tooltips** on the same **Bar Surface**.
- Niribar shows at most one **Menu** at a time.
- A **Menu** closes when its originating visible target disappears.
- Opening a **Panel** closes any open **Menu**.
- Opening a **Menu** closes any open **Panel**.
- **Keyboard Focus** enters a **Menu** while it is open and returns to the originating target when the **Menu** closes, if that target still exists.
- **Menus** and **Tooltips** do not inherit the **Layer** of their **Bar Surface**.
- Niribar shows at most one **Panel** at a time.
- A **Panel** belongs to exactly one originating **Bar Surface**.
- A **Panel** has exactly one **Panel Origin**.
- A **Panel Origin** may be the **Bar Surface** itself or a **Module** on that **Bar Surface**.
- A **Panel** does not change the size, **Edge Span**, or normal module rendering of its originating **Bar Surface**.
- A **Panel** renders as an overlay and does not reserve exclusive screen space.
- A **Panel** opens beside the **Edge** of its originating **Bar Surface**.
- A module-originated **Panel** aligns to the originating **Module** when its bounds are available and falls back to the **Bar Surface** otherwise.
- The **Panel Origin** remains visually marked while the **Panel** is open.
- A **Panel** closes when its originating **Bar Surface** disappears.
- A **Panel** closes when keyboard focus leaves the **Panel** for another application or window.
- A **Panel** may remain open when focus moves from the originating **Bar Surface** into the **Panel**.
- Escape closes a **Panel** unless the active **Panel** content has a more specific first-Escape behavior.
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
- **Panel Menu Search** uses fuzzy matching for configured **Panel Menus**, installed-applications **Panel Menu Sources**, and **Dmenu Panel Menu Sources**.
- **Panel Menu Search** uses fuzzy matching as the default behavior rather than offering a substring-search mode.
- **Panel Menu Search** fuzzy matching requires query characters to appear in order.
- **Panel Menu Search** is ASCII case-insensitive.
- **Panel Menu Search** splits search text on whitespace and requires every query token to match.
- **Panel Menu Search** ranks matching entries by match quality and uses source order as the tie-breaker.
- **Panel Menu Search** ranks substring matches ahead of non-contiguous fuzzy matches when other priority rules are equal.
- **Panel Menu Search** preserves source order when the search text has no non-whitespace tokens.
- **Panel Menu Search** returns all matching entries rather than applying a hidden result cap.
- **Panel Menu Search** ranks visible leaf labels ahead of secondary metadata, and secondary metadata ahead of breadcrumb-only ancestor matches.
- **Panel Menu Search** can match breadcrumb ancestor labels for nested configured **Panel Menu** entries as lowest-priority text.
- **Panel Menu Search** uses Niribar-owned matching behavior rather than exposing a third-party matcher as domain behavior.
- **Panel Menu Search** for **Installed Desktop Applications** matches application names, descriptions, generic names, and keywords.
- A dynamic **Panel Menu Source** is refreshed when its **Panel Menu** opens and remains stable while that **Panel** is open.
- A **Panel Menu** may contain nested items, but the **Panel** displays one active menu level at a time unless search is active.
- **Panel Menu Search** matches nested items and displays matching results with breadcrumb context.
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
- **Dmenu Selection Mode** allows the user to submit arbitrary typed text by default.
- **Dmenu Selection Mode** can be explicitly restricted to provided entries with `--no-custom`.
- Empty input with `--no-custom` offers no selectable entries and can only be cancelled.
- When **Dmenu Selection Mode** has a focused matching entry, normal activation returns that focused entry.
- Submitting arbitrary typed text uses a distinct custom-value affordance.
- In **Dmenu Selection Mode**, a custom typed value appears after fuzzy-matched entries, or first when there are no matching entries.
- **Dmenu Selection Mode** writes the selected entry and does not run configured **Panel Actions**.
- **Dmenu Selection Mode** exits successfully only when the user selects or submits a value.
- Cancelling **Dmenu Selection Mode** exits distinctly from CLI usage errors and IPC/request failures.
- **Dmenu Selection Mode** has no default timeout while waiting for user selection.
- If the running Niribar instance disconnects before completing a **Dmenu Selection Request**, the CLI treats it as an IPC/request failure.
- A **Dmenu Selection Request** opens on the focused **Output** when known, or on one deterministic **Bar Surface** when no focused **Output** is known.
- Opening a **Dmenu Selection Request** closes any existing **Panel** or **Menu**.
- A running **Dmenu Selection Request** rejects additional **Dmenu Selection Requests** until it completes.
- A **Dmenu Selection Request** cancels when its **Panel** loses focus, consistent with other **Panels**.
- A **Dmenu Selection Request** is not a configured **Panel Action**.
- A **Dmenu Selection Request** is displayed as a **Panel Menu** with a **Dmenu Panel Menu Source**.
- A **Dmenu Panel Menu Source** returns its activated value to the requesting CLI process instead of running a configured **Panel Action**.
- A **Dmenu Panel Menu Source** initially treats each input entry as both the visible label and returned value.
- Plain **Dmenu Entry Streams** treat each record as both the visible label and returned value.
- Rich **Dmenu Entries** keep the candidate record boundary line-oriented while allowing the visible label and returned value to differ.
- A value-only **Rich Dmenu Entry** uses its returned value as its visible label.
- Selecting a **Rich Dmenu Entry** returns its value to the caller; if no separate value is present, it returns the visible label.
- A custom typed value in **Dmenu Selection Mode** is returned as typed and is not a **Rich Dmenu Entry**.
- **Dmenu Selection Mode** keeps its selection-value semantics when using **Panel Menu Search**.
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
