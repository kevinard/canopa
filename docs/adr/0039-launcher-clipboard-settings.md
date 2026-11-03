# Launcher clipboard history and Settings

Date: 2026-11-03
Status: accepted

Shell owns clipboard capture, in-memory history, selection and presentation in
the existing Applications Panel Menu. Settings edits recording, image capture
and the entry limit on the existing Launcher page. Image capture is a separate
opt-in and depends on recording. Disabling it drops recent images. Disabling
recording drops all recent entries.

The shared canopa-launcher clipboard store holds preferences and explicitly
saved text snippets in the existing owner-only state file. Existing stores
default to image capture off and a limit of 64. Shell and Settings take the same
lock and reread before writing. Settings applies an options-only edit and rejects
drafts based on older options. Shell applies live saves and observes external
changes off-thread. Each Shell save retires queued reads of the previous state.
Capture-mode changes retire pending observations.

Shell uses separate bounded wl-paste subscriptions for text and images. It
filters selection state before reading, validates supported raster formats and
prepares thumbnails off the UI thread. The original encoded bytes, thumbnails
and recent text are session data. Selecting an image hands its bytes to wl-copy
using the corresponding MIME type.
