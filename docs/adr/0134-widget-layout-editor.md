# 0134 Widget layout editor

Status: accepted.

Canopa Settings edits the existing Shell bar zones and the existing lockscreen
regions. It keeps a draft of the common KDL file and publishes it through the
same snapshot checks and owner validation used by Personalization. The first bar
edit materializes the existing embedded Shell defaults if the common file
omits the Shell namespace. Existing bar actions, styles, module declarations,
panel layouts and unrelated configuration remain with their owners.

Personalization and Widgets edit the same file with separate drafts. When one
page saves, the other keeps its pending edits on top of the new file, provided
both drafts came from the same version and the save left the other page's
sections unchanged. Any other difference is an external edit and still requires
reloading that page.

Each bar can be placed on all outputs or use an exact connector inclusion or
exclusion list. Duplicating a bar creates a separate layout and starts with an
empty inclusion list. Shell applies accepted configuration with its current
live reload. Per-output lockscreen visibility uses an optional `hide-widgets`
list in the corresponding background override. A new lock session uses the
saved layout. Authentication appears once in every layout and is ineligible for
output hiding. The single lock accessibility tree omits labels of passive
widgets hidden on any configured output.

The editor shows a diagram with sample content and a scale control. It uses the
lockscreen layout planner for regions and keeps the Shell preview schematic.
Previewing reads layout data and does not start providers, custom commands or
an authentication transaction. Pointer selection and keyboard controls operate
on the same draft. Providers, greeter and lock authentication remain
independent of Settings.
