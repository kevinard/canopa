# Canopa Files context

Canopa Files is the desktop's independent local file manager. It owns directory
browsing, file selection, searches, previews, custom actions, preferences, file
operations, and Undo. It consumes Canopa's shared appearance and UI treatment;
Shell and Notify retain their established ownership and lifetimes.

## Language

- A Browser is one location-navigation session, hosted in a Tab.
- A Browser Window hosts Tabs and owns the origin of its Quick Look Window.
- A Directory is an immutable filesystem snapshot prepared off the UI thread.
- A Device is a local volume or standalone local mount exposed by the platform.
  Mount and unmount requests have their own lifetime and do not enter file Undo.
- A Device Request is a backend-owned mount or removal attempt. Its current
  question has an opaque answer token; descriptive results do not grant current
  root navigation or restart execution authority.
- Selection contains the focused file, selected files, and the range anchor.
- Quick Look follows its originating Browser Window, including its active Tab,
  independently of focus in other Browser Windows. The auxiliary Trash window
  and Recovery windows own their previews through the same source interface.
- An Operation is a user request such as a batch copy, move, rename, or trash.
- A Transfer is the asynchronous execution of an Operation. It can outlive all
  Browser Windows and exposes progress, conflicts, failures, and cancellation.
- A Source Lease protects catalogued retained data while an owned clipboard
  selection or drag can still use it. It grants retention, not transfer execution.
- A Drag owns one captured selection and its live gesture state. Prepared hover
  destinations share that gesture; cancelling one hover does not cancel another.
- A Drop captures file references and a destination from a drag gesture; its
  resolved Copy/Move action becomes a normal Transfer request.
- An Undo Entry describes actual completed changes and the information needed
  to reverse them. Undo history is shared across Browser Windows and Tabs.
- An Undo Relocation records a restored item's new path so older history still
  refers to that item, including its descendants and retained replacement data.
- Recovery lists retained versions still needed by Undo and protected data that
  requires explicit recovery. A Recovery Copy creates an undoable copy at a new
  path without consuming the retained version.
- An Interrupted Operation retains a filesystem journal that cannot yet be
  reconciled. Recovery exposes its reason and owned files while normal mutations
  wait. Recovery copies have a separate journal and leave that record intact.
- A Custom Action is an explicitly invoked command or script with selection
  conditions. It does not run automatically as files arrive.

## Public test seams

Behavioral tests exercise Directory and Browser, the transfer engine, Trash,
preferences, the custom-action queue, portable storage requirements, durable scheduling records, preview
preparation, and custom-action execution through their public interfaces.
Scheduling-record tests cover persistence; native queue restoration and
filesystem-step crash continuation have their own acceptance gates (ADR0056).
Configuration tests use immutable snapshots and the serialized
reload runtime against actual isolated source/theme files. Native desktop
acceptance exercises window focus, tiling, keyboard input, clipboard, drag and
drop, and rendered appearance in the repository's private desktop harness.
Local-volume acceptance replaces the external GIO storage provider with a private
module, retaining real temporary files, native signals and asynchronous requests.
It never changes an actual user mount or block device.

See [the product brief](../../.scratch/file-manager/PRD.md),
[the implementation ledger](../../.scratch/file-manager/IMPLEMENTATION.md), and
[application decisions](adr/0001-application-and-state.md).

## Decision index

- [Saved-only Rename restoration retains original history binding](adr/0157-saved-only-rename-history-binding.md).

- [XXH3-128 content fingerprints without development-history compatibility](adr/0156-fast-content-fingerprints.md).

- [Trust upstream Copy and fingerprint its output once](adr/0155-trust-upstream-copy.md).

- [Hash fully compared Copy streams once, preserving verification](adr/0154-verified-copy-hashing.md).

- [uutils descriptor copying in the shared prepared-copy lifecycle, scoped migration](adr/0153-uutils-prepared-copy.md).

- [Proved saved-return refusal and paired saved-only history, schema60 verification in progress](adr/0152-proved-saved-return-refusal.md).

- [Original Undo displacement preparation, schema59 focused verification in progress](adr/0151-original-undo-displacement-preparation.md).

- [Selected saved returns consume their original name once, schema58 implementation](adr/0150-selected-saved-return-restart.md).

- [Changed-copy preservation binds its original Undo history, schema57 diagnosis retains two Undo Replace failures](adr/0149-changed-copy-preservation-restart.md).

- [Saved-destination Copy Undo separates return from payload disposal, scoped schema56 verification](adr/0148-saved-copy-undo-restart.md).

- [Original Copy Undo consumes each namespace once, scoped schema55 verification](adr/0147-original-copy-undo-restart.md).

- [Completed Undo retains cleanup of original work folders, scoped schema54 verification](adr/0146-completed-undo-cleanup.md).

- [Durable compensation of interrupted Undo Replace, schema53 implementation](adr/0145-displaced-undo-compensation.md).

- [Bound displaced Undo preparation and retention, scoped schema52 verification](adr/0144-displaced-undo-retention.md).

- [Verified disposal of interrupted Undo returns, scoped schema51 verification](adr/0143-undo-partial-disposal.md).

- [Bound Undo return work and explicit retention, schema50 verified foundation](adr/0142-undo-return-work-retention.md).

- [Nested created-folder continuation and Recovery access, scoped schema49 verification](adr/0141-nested-recovery-access.md).

- [Recovery access beneath restricted created folders, scoped schema49 verification](adr/0140-recovery-folder-read-access.md).

- [Reproducible partial-copy disposal, scoped schema48 verification](adr/0139-reproducible-partial-disposal.md).

- [Reviewed disposal retention, scoped schema47 verification](adr/0138-reviewed-disposal-retention.md).

- [Preparation disposal namespaces, scoped verification; public Keep review unfinished](adr/0137-preparation-disposal-namespaces.md).

- [Preparation and cleanup task bindings, scoped verification; filesystem disposal unfinished](adr/0136-preparation-and-cleanup-task-bindings.md).

- [Copied saved-container restart claims, scoped verification](adr/0135-copied-saved-container-restart-claims.md).

- [Saved-copy descriptors and created target ownership, scoped foundation](adr/0134-saved-copy-descriptors-and-created-ownership.md).

- [Saved-container whole-Move restart tasks, scoped verification](adr/0133-saved-container-whole-move-restart-tasks.md).

- [Reviewed replacement restart tasks, scoped verification](adr/0132-reviewed-replacement-restart-tasks.md).

- [Whole-entry Move restart tasks, scoped verification](adr/0131-whole-entry-move-restart-tasks.md).

- [Source-directory removal restart recovery, scoped verification](adr/0130-source-directory-removal-restart-recovery.md).

- [Directory expansion restart recovery, scoped verification](adr/0129-directory-expansion-restart-recovery.md).

- [Recovered transfer-end Retry Save ownership, scoped verification](adr/0128-recovered-transfer-end-save-ownership.md).

- [Returned-source task dispositions and original cleanup continuation, scoped verification](adr/0127-returned-source-task-dispositions.md).

- [Final-leaf restart completion and durable folder reports, scoped verification](adr/0126-final-leaf-restart-completion.md).

- [Copied-leaf restart step proofs and original source cleanup, scoped verification](adr/0125-copied-leaf-restart-steps.md).

- [Active transfer traversal cursors, scoped verification; filesystem-step continuation unfinished](adr/0124-active-transfer-traversal-cursors.md).

- [Restored stopped-transfer execution owners, scoped verification; active/queued restart unfinished](adr/0123-restored-stopped-transfer-owners.md).

- [Empty-source-directory claims, scoped verification; restart ownership unfinished](adr/0122-empty-source-directory-claims.md).

- [Ordinary prepared-copy publication, scoped verification; restart ownership unfinished](adr/0121-ordinary-prepared-copy-publication.md).

- [Forward intent and completed cleanup rights, scoped verification; restart ownership unfinished](adr/0120-forward-intent-and-completed-cleanup-rights.md).

- [Reviewed replacement claims, scoped verification; restart execution ownership unfinished](adr/0119-reviewed-replacement-claims.md).

- [Whole-entry Move claims, scoped verification; restart execution ownership unfinished](adr/0118-whole-entry-move-claims.md).

- [Opened source-removal claims, implementation and verification in progress](adr/0117-opened-source-removal-claims.md).

- [Opened copied leaves and stages, implementation in progress; restart execution unfinished](adr/0116-opened-copy-leaves-and-stages.md).

- [Logical history-lock lifetime, release-race regression verified](adr/0115-logical-history-lock-lifetime.md).

- [Opened transfer folder facts, implementation in progress; restart execution unfinished](adr/0114-opened-transfer-folder-facts.md).

- [Exact stopped transfer results, scoped backend and desktop-library verification](adr/0113-recorded-stopped-transfer-results.md).

- [Durable selected-entry facts, implementation in progress; restart continuation gate still failing](adr/0112-durable-selected-entry-facts.md).

- [Exact recorded Copy/Move completion, scoped backend/desktop/native verification](adr/0111-recorded-transfer-completion.md).

- [Exact recorded ordinary mutation completion, scoped backend/desktop/native verification](adr/0110-recorded-mutation-completion.md).
- [Keep interrupted Rename claims in Recovery, scoped backend/Iced/native verification](adr/0109-keep-interrupted-rename-in-recovery.md).
- [Ordinary Rename through opened parents and journalled entry claims, scoped backend/Iced/native verification](adr/0108-opened-parent-rename-claims.md).

- [Prepare Rename authority before its name draft](adr/0106-prepared-rename-authority.md).
- [History writer acquisition before mutation checks](adr/0107-history-write-transaction-ordering.md).

- [Queued retained Rename ownership and reviewed request dismissal, scoped backend/Iced/native verification](adr/0105-retained-rename-ownership-and-dismissal.md).

- [Exact request-to-filesystem execution evidence, scoped verification; restart incomplete](adr/0104-request-filesystem-execution-evidence.md).

- [Queued Rename selected-entry and parent observations, scoped backend/native verification](adr/0103-queued-rename-observations.md).

- [Backend-owned live transfer Retry, Continue and abandonment, scoped backend/Iced verification](adr/0102-live-application-transfer-continuation.md).

- [Original destination folders through live continuation, scoped backend/native verification](adr/0101-live-transfer-destination-observation.md).

- [Original selected sources through live transfer continuation, scoped core/native verification](adr/0100-live-continuation-selected-sources.md).

- [FileManager1 presentations, grouped reveal and read-only Properties, scoped core/native verification](adr/0099-file-manager-presentations.md).

- [Quick Look dismissal returns to its focused owner; complete Downloads workflow and scoped native regression verified](adr/0098-quick-look-dismissal-focus.md).

- [Drag destination observations survive refresh; scoped backend and native verification](adr/0097-drag-destination-observations-survive-refresh.md).

- [Search shortcuts preserve query editing; scoped native regression](adr/0096-search-shortcuts-preserve-query-editing.md).

- [Iced drag gestures, device targets, cross-filesystem choices, GTK exchange and windowless receipts; scoped native verification](adr/0095-iced-drag-gestures-and-transfer-lifetime.md).

- [Shared Iced native drag transport, scoped evidence and compositor compatibility](../adr/0023-wayland-drag-transport.md).

- [Niri dead-source grab correction, private native companion verified; distribution pending](../adr/0024-niri-drag-source-lifetime.md).

- [Backend-owned drag capture and drop admission](adr/0094-backend-owned-drag-capture-and-drop-admission.md).

- [Managed custom-action storage, scoped backend and native verification](adr/0092-managed-custom-action-storage.md).

- [Iced device controls and floating questions, scoped workflow verification](adr/0093-iced-device-controls-and-questions.md).

- [Backend-owned device requests, current-root lookup and attention, scoped verification](adr/0091-backend-owned-device-catalog-and-commands.md).

- [Renderer-independent Iced media playback and common formats, scoped native verification](adr/0090-renderer-independent-media-playback.md).

- [Prepared Iced source highlighting and native preview coverage, scoped verification](adr/0089-iced-prepared-source-highlighting.md).

- [Interrupted Recovery startup attention and native reconciliation, scoped verification](adr/0088-interrupted-recovery-startup-attention.md).

- [Recovery catalog and Iced window, retained-file and directory continuation verified; broader acceptance pending](adr/0087-iced-recovery-catalog-and-window.md).

- [Backend-owned Recovery copies and reconciliation, scoped verification; catalog and Iced integration pending](adr/0086-backend-owned-recovery.md).

- [Iced Trash catalog and window, scoped Main Trash workflow verified](adr/0085-iced-trash-catalog-and-window.md).

- [Backend-owned Trash and Restore, backend workflow verified](adr/0084-backend-owned-trash-and-restore.md).

- [Resident host and local activation, scoped verification](../adr/0022-files-resident-host-and-activation.md).

- [Backend-owned filesystem attention, scoped verification](adr/0083-backend-owned-filesystem-attention.md).

- [Backend-owned custom-action startup and attention, scoped verification](adr/0082-backend-owned-action-attention.md).

- [Backend-owned volume and operation startup, scoped verification](adr/0081-backend-owned-operation-startup.md).

- [Core-owned configuration coordination, scoped verification](adr/0080-core-owned-configuration-coordination.md).

- [Bounded owned-clipboard replacement capacity, scoped verification](adr/0079-owned-clipboard-replacement-capacity.md).

- [Core-owned custom-action queue, scoped verification](adr/0078-core-owned-custom-action-queue.md).

- [Core-owned prepared Copy/Cut workflow, scoped verification](adr/0077-core-owned-prepared-clipboard.md).

- [Window-independent runtime and optional resident mode, scoped hosting verified; extraction incomplete](adr/0076-window-independent-and-resident-runtime.md).

- [Process-owned selected-source observations, in progress](adr/0075-selected-source-observations.md).

- [Renderer-independent application runtime, implementation incomplete](adr/0074-renderer-independent-application-runtime.md).

- [Retained payload mutation ordering, scoped verification](adr/0073-retained-payload-mutation-ordering.md).

- [Process-owned retained-source leases, in progress](adr/0072-process-owned-retained-source-leases.md).

- [Iced transfer progress and result inspection, in progress](adr/0071-iced-transfer-progress-and-result-inspection.md).
- [Iced transfer questions and Cut completion, in progress](adr/0070-iced-transfer-questions-and-cut-completion.md).
- [Iced native file clipboard, in progress](adr/0069-iced-native-file-clipboard.md).
- [Durable retained-source ownership, in progress](adr/0068-durable-retained-source-ownership.md).
- [Iced shared transfer service, in progress](adr/0067-iced-transfer-service.md).

- [Application ownership, state and keyboard defaults](adr/0001-application-and-state.md).
- [Browsing preferences, persistence and filename search](adr/0002-browsing-preferences-and-search.md).
- [Custom action arguments, conditions, output and cancellation](adr/0003-custom-action-execution.md).
- [Historical GTK choice, superseded](../adr/0015-files-native-widgets.md).
- [Iced and shared controls; supersedes the GTK choice](../adr/0018-files-iced-and-shared-controls.md).
- [Native browsing and bounded previews](adr/0004-native-browsing-and-previews.md).
- [Persistent operation history](adr/0005-operation-history.md).
- [Transfer transactions and retained replacements](adr/0006-transfer-transactions.md).
- [Native file clipboard and shared Operations](adr/0007-native-clipboard-and-operations.md).
- [Trash, restore and recovery](adr/0008-trash-and-restore.md).
- [Custom action editor, shared configuration and controls](adr/0009-custom-action-editor.md).
- [File drag and drop](adr/0010-drag-and-drop.md).
- [Shared interface preferences and deliberate folder choices](adr/0011-interface-preferences.md).
- [Interactive Undo and restoring under a new name](adr/0012-interactive-undo.md).
- [Protected retained versions and Recovery copies](adr/0013-protected-data-recovery.md).
- [Undo Replace preserves newer files in Recovery](adr/0014-undo-replace.md).
- [Undo restores previous destinations](adr/0015-undo-previous-destinations.md).
- [Index affected Undo and Recovery namespaces](adr/0016-undo-namespace-index.md).
- [Interrupted operations stay inspectable](adr/0017-interrupted-operation-recovery.md).
- [Undo changed copies while preserving newer work](adr/0018-undo-changed-copies.md).
- [Live configuration and appearance reload](adr/0019-live-configuration-reload.md).
- [Local volumes and explicit device operations](adr/0020-local-volumes.md).
- [Storage placement and queued request intent](adr/0021-storage-placement-preflight.md).
- [Running transfer placement and safe pause](adr/0022-live-transfer-placement.md).
- [Preview control focus and media playback](adr/0023-preview-control-focus-and-media.md).
- [Media previews retain the opened file](adr/0024-preview-opened-media.md).
- [Media format acceptance and bounded ID3 input](adr/0025-media-formats-and-id3.md).
- [Review changed saved versions before Undo](adr/0026-undo-changed-saved-versions.md).
- [History readers and Recovery during live questions](adr/0027-history-readers-and-live-recovery.md).
- [Undo of replaced moved or renamed items](adr/0028-undo-replaced-moved-items.md).
- [Undo when a moved item cannot be inspected](adr/0029-undo-unavailable-moved-items.md).
- [History identity ownership](adr/0030-history-identity-ownership.md).
- [Undo with unavailable saved versions](adr/0031-undo-unavailable-saved-versions.md).
- [Original-location conflicts during cross-filesystem Undo](adr/0032-cross-filesystem-undo-original-conflicts.md).
- [Changed and unavailable cross-filesystem move destinations](adr/0033-cross-filesystem-undo-destination-review.md).
- [Hidden-folder hierarchy and parent selection](adr/0034-hidden-folder-navigation-context.md).
- [Nonempty created-folder Undo](adr/0035-nonempty-created-folder-undo.md).
- [Changed and unavailable created-folder locations](adr/0036-changed-and-unavailable-created-folders.md).
- [Directory-return Recovery namespaces](adr/0037-directory-return-recovery-namespaces.md).
- [Persistent directory Merge returns](adr/0038-directory-merge-return-sessions.md).
- [Remaining Merge work and interruption proof](adr/0039-merge-remainders-and-interruption-proof.md).
- [Additional Merge items and owned destination containers](adr/0040-additional-merge-items-and-owned-destinations.md).
- [Changed and unavailable folders in a partial Merge](adr/0041-partial-merge-folder-review.md).
- [Merge with unavailable saved previous destinations](adr/0042-merge-with-unavailable-saved-versions.md).
- [Cross-filesystem Merge children, in progress](adr/0043-cross-filesystem-merge-children.md).
- [Decomposed directory-move Undo, in progress](adr/0044-decomposed-directory-move-undo.md).
- [Directory retry completion plans and cut intent](adr/0045-directory-retry-completion-plans.md).
- [Saved-stage cross-filesystem returns, in progress](adr/0046-saved-stage-cross-filesystem-returns.md).
- [Saved versions during folder transfers, in progress](adr/0047-saved-versions-during-folder-transfers.md).
- [Recovery context authority](adr/0048-recovery-context-authority.md).
- [Shared Recovery expiry](adr/0049-shared-recovery-expiry.md).
- [Directory Recovery copies, in progress](adr/0050-directory-recovery-copies.md).
- [Recovery continuation ownership](adr/0051-recovery-continuation-ownership.md).
- [Operations in narrow tiles](adr/0052-operations-narrow-tiles.md).
- [Copied-folder permissions and temporary access](adr/0053-folder-permissions.md).
- [Retained-replacement expiry and captured parent authority](adr/0054-retained-replacement-cleanup.md).
- [Clipboard source placement and Retry](adr/0055-clipboard-source-placement.md).
- [Durable unfinished operations, in progress](adr/0056-durable-unfinished-operations.md).
- [Iced directory loading and bounded row construction](adr/0057-iced-directory-loading.md).
- [Iced filename search and tab-owned results](adr/0058-iced-filename-search.md).
- [Iced source-bound floating previews](adr/0059-iced-source-bound-previews.md).
- [Iced custom actions and shared configuration, in progress](adr/0060-iced-custom-actions.md).
- [Iced editing state and optional action icons](adr/0061-iced-editing-state-and-action-icons.md).
- [Iced folder views and private preferences](adr/0062-iced-folder-views-and-preferences.md).
- [Iced global interface preferences, in progress](adr/0063-iced-global-preferences.md).
- [XDG personal-folder locations](adr/0064-xdg-personal-folder-locations.md).
- [Iced filesystem operation interfaces, in progress](adr/0065-iced-filesystem-operation-interfaces.md).
- [Iced name dialogs and live volume observation, in progress](adr/0066-iced-name-dialogs-and-volume-observation.md).
- [File manager distribution and runtime dependencies](../adr/0017-files-distribution-and-runtime.md).
