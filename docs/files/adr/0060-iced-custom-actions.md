# Iced custom actions and shared configuration

Status: accepted under delegated implementation authority, 2026-10-18.
Implementation and native acceptance are in progress. The full product remains open.

Port custom actions to the same Iced/canopa-ui interface as browsing and Quick
Look. Preserve the argument, condition, process-group, draft and stale-save
contracts in ADR 0003 and ADR 0009. The GTK adapter is migration reference only.

One application-owned Custom Actions editor serves all Browser Windows. Its
catalog draft retains every action while switching selection. Stable entry IDs
and selection epochs guard queued field events. Save validates all commands,
conditions and canonical shortcuts against the actual built-in bindings. Reload
and closing a changed draft require an explicit discard choice; Keep editing is
the default. Saving keeps the application alive until the writer acknowledges it.
Source bytes and target checks in the existing configuration writer remain the
save authority, including symlinks and external text-editor changes.

One admitted configuration worker serializes explicit reads, saves and external
source polling. It owns the existing ConfigurationRuntime and publishes a
monotonic revision with a complete valid snapshot or a rejection. The latest
snapshot occupies one mailbox and its notification channel is bounded. Save
acknowledgements have their own one-shot replies and cannot be coalesced away.
A rejected external edit keeps the complete last valid configuration. Applying
appearance/settings never resets the editor draft or the preview's file/page/zoom.
Old acknowledgements cannot replace a newer published configuration.

Every action invocation captures its catalog version, originating Tab/navigation,
selection paths and working-folder binding. A queued control from a superseded
selection or catalog cannot run a different command. Once accepted, a job keeps
that captured request independently of subsequent navigation or editor changes.
Presentation uses cached entries; its worker rechecks applicability and the
working folder's canonical binding before preparing the existing argument-safe
command. Full volume observations and durable source leases remain an open
integration gate; canonical-path equality alone is not that authority. No command runs
from configuration reload or file arrivals.

Use one shared Operations presentation for jobs. Custom commands can run alongside
future serialized filesystem transfers; at most four command workers run at once.
[ADR0078](0078-core-owned-custom-action-queue.md) moves queue ownership into the
renderer-independent core and refines the capacity to 1024 slots shared between
unfinished requests and compact completion receipts not yet handed to the host.
Retain 100 finished output summaries. Output remains
bounded to the executor's 64 KiB per stream and is selectable in Operations.
Cancellation terminates the owned process group. Closing Browsers does not cancel
accepted commands or drop their application lifetime. Arbitrary command effects
never create fabricated Undo entries and do not depend on writable Undo storage.

The Iced adapter must eventually connect the durable shared scheduling contract
in ADR 0056; a visible in-memory row alone is not restart continuation or proof of
that gate. External commands must never be automatically replayed after restart.
File transfers, durable source leases, devices and actual file exchange retain
all their existing implementation and acceptance requirements.

Configured toolbar actions and context/keyboard invocation must remain usable in
narrow tiles. Optional icons use shared bounded asset lookup/preparation, not a
second widget toolkit. The editor uses shared controls and a vertically scrolling
form with persistent Save/Reload access. Record actual native acceptance in both
themes, including raw paths, mixed selections, stale drafts, output/cancellation,
background lifetime and preservation of source navigation. Do not check off the
complete custom-action/product requirement before all its gates are established.

Toolbar configuration determines the primary order, including explicit custom
action positions. Each command has a bounded clipped width and a full-label
tooltip; only the location field grows. When that exact width budget cannot fit,
the location occupies a separate top row and the commands retain their relative
order in a horizontal scroll strip below. Additional actions marked for the
toolbar appear in catalog order in a second horizontal strip. Resizing does not
change any command or keyboard meaning. Both built-in and custom toolbar events
carry the selection/catalog token they described. Controls for interfaces still
being ported remain disabled until their behavior is implemented and verified.

Editor and Operations keyboard traversal uses a window-owned widget group.
Buttons and checkboxes expose Iced focusable state through a thin wrapper using
Canopa's shared focus border; text fields retain their own native widget state.
The group handles Tab/Shift+Tab synchronously: it inspects actual focus, changes
focus in its own subtree, and reveals the chosen control before returning from
the event. Space/Return activates a focused control in that widget handler. This
preserves batched Tab, Ctrl+A and typing without asynchronous app callbacks.
Disabled controls are excluded from traversal; discard prompts start on Keep
editing. Mouse clicks share the same focus state and do not affect other windows.

The earlier app-message queries dropped rapid Tab steps. A serialized intent
counter passed controller tests but still let Ctrl+A arrive before native focus,
appending to the action name in native12. That approach is superseded by the
synchronous widget group. Native verification must include batched keys and exact
saved field values, not only a delayed model-level traversal test.

Operations output remains read-only while allowing selection and copying; each
job can open its captured working folder in a new Browser. Viewport shrinkage
reveals a previously visible focused file with the minimum scroll adjustment,
while preserving intentional scrolling away from that file.

Native diagnosis found that laying out both complete 64 KiB streams with long
unbroken lines delayed painting by 3.72 seconds in the debug renderer. Keep the
complete bounded captured output, but lay out at most 8 KiB at a time, with page
navigation and Copy all output. Page boundaries preserve Unicode and every
character remains available. The UI must not wait for full output layout before
responding to subsequent keys. Native acceptance must wait for actual painted
state rather than assuming a keypress has already produced a frame.

Toolbar tooltips are suppressed while a location/search field is active and while
the originating context menu is open. They must not obscure editable controls.
