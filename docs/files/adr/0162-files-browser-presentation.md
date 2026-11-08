# 0162 Files browser presentation

The browser uses Canopa's shared appearance roles and installed typography for
its toolbar, Places sidebar, folder columns and status bar. Quiet highlights
identify the ancestor trail. The current folder has a tinted heading, and its
focused row retains the configured focus treatment. The immediate parent shares
spare width with the current folder. Older columns retain the configured width.
Row geometry and virtualization use the same padding and heights.

The toolbar preserves the configured command order. The location slot shows
clickable ancestors and the current folder, then collapses the oldest visible
segments as space runs out. The current segment, pencil and existing location
shortcut open the native location editor, which retains the full path and
byte-safe file URI handling. The toolbar keeps its editor subtree across
sidebar and compact layout transitions. The toolbar keeps actions labeled where
space permits. Narrower tiles use an icon for Search or keep a separate labeled
command strip. Space opens Quick Look, which remains available through the
browser's context menu and shortcut preferences. Existing toolbar entries for
Quick Look are ignored when reading a configuration.

When the sidebar is hidden, a Places button opens the existing Locations menu
with the configured destinations and devices. The sidebar keeps device
navigation and removal separate. File symbols use the existing bounded asset
worker, with symbolic fallbacks for folders, links, images, PDF, archives and
other files. Format cues are derived from the entry kind and extension and have
no authority over file activation. Long labels can be shortened in the row and
revealed in full on hover. Operations, selections, drag targets and breadcrumb
messages retain their original paths.

Secondary windows use titled headers, grouped metadata and shared action
treatments. Properties separates file identity from labeled facts, while its
Copy locations action retains the original local URI. Trash and Recovery use
Space for previews and show an empty state in the listing. Operation and
device prompts preserve their existing focus, request ownership and replies.

Operations uses a shared row layout for file history. Completed rows identify
their result, and transfer inspection uses the same status treatment. Undo
appears on a history row when its retained report identifies the current Undo
entry. The global action remains available for other entries. The request
catalogue records the admission time in the same transaction as a new request.
Operations can display that time after restart. Older requests retain an
unknown admission time. The timestamp is presentation data, separate from
request identity and execution authority.

Conflict dialogs align the incoming and existing facts and show the choice
scope before the footer. Recovery describes saved data by its role in the
request. The underlying paths, selection, request generation and default Cancel
behavior retain their existing authority. Restore and Recovery forms keep
persistent field labels and a separate action footer.

Both destination drafts offer a folder browser beside the path field. Folder
navigation uses the same directory snapshots as the Browser on a cancellable
worker. Confirming a folder updates the captured draft and keeps the operation
pending until the user submits the Restore or Recovery action.

Wide Browser windows show view modes in the toolbar. Narrow windows keep the
same commands in the status bar. List places configured details in aligned
columns when space permits and below each name in a narrow window. Row height
changes preserve the scroll anchor and focused selection.

Wide Trash and Recovery windows give the catalogue the full width until an
item is selected. Details and actions then appear beside the list, preserving
its scroll position. Compact windows place item actions below the listing.
Quick Look uses a smaller initial audio window and keeps the seek and volume
controls grouped at narrow widths.
