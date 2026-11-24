# 0158. Share Niri window previews between Shell and portal pickers

Status: accepted

## Context

Niri window titles can be identical or missing. Shell already renders the
current contents of a window in taskbar tooltips, using Niri's window-preview
service. Portal source pickers need the same image to let the user identify
a window before granting capture.

## Decision

Share the D-Bus client in `canopa-services` and the compositor app ID
matcher in `canopa-apps`. Resolve the preview service's unique bus owner.
For portal pickers, verify that it belongs to the process that supplied the
source catalog. Retrieve small PNG snapshots for windows near the visible
list, and use the app icon and window title when a preview is unavailable.

## Consequences

Preview pixels remain in the dialog process. Selecting and granting capture
continues to depend on the existing source identity and consent flow. Disabling
the Niri preview service turns off thumbnails in both Shell and the pickers.
