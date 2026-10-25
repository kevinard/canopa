# Native browsing and bounded previews

Status: accepted under delegated implementation authority, 2026-10-16.

Browser Windows use GTK Application windows and one independently owned Browser
per Tab. GApplication forwards local paths and file URIs to the running Files
process; multiple initial paths become tabs in a new window. Non-local URIs are
rejected. The location field displays paths lossily but preserves the original
path if its displayed value is submitted unchanged.

The sidebar appears from 960 logical pixels. Ancestor columns collapse from the
left at the configured readable width; the current folder fills the narrow
window. Ancestors are prepared asynchronously. Native list and grid models
virtualize rows; no directory enumeration or preview decoding runs in a row
factory. Directory monitors refresh real filesystem changes. Request revisions
discard stale browsing, refresh, search and preview results.

Selection and its range anchor belong to Browser. The native adapter reconciles
pointer selection while preserving that anchor and synchronizes keyboard
selection to the widget. Focusing a virtual row is insufficient when a column
has been rebuilt: the list must also receive keyboard focus. Native acceptance
reproduced this missing-focus problem and protects the correction. Editable
fields retain text editing keys; Down or Enter moves from filename search into
its results. Closing search restores the prior selection and scroll position.

The folder view menu exposes sorting, reverse order, readable column width,
visible details, Use defaults, and reveal in the containing folder. Deliberate
changes are folder overrides; saving them holds the application alive until the
atomic write finishes. A larger interface editor and shared Files configuration
namespace are still separate, unfinished requirements.

Quick Look is a non-modal transient window attached to its originating Browser
Window, not a tile or a preview column. Its default size is 760 × 600 logical
pixels. It follows that window's active Tab, including while another Browser
Window is focused. Space or Escape closes it and returns focus to its browser.
Navigation keys in Quick Look continue browsing through the source window.

Preview work uses cancellation and independent request revisions. Opening
special files never waits for a writer: the worker opens nonblocking and
validates the descriptor as a regular file. Encoded image/PDF input is limited
to 128 MiB; image dimensions to 16,384 pixels per axis and decoder allocation to
256 MiB. PNG, JPEG, GIF and WebP are enabled. Images have fit, actual-size and
zoom controls. Unsupported formats offer opening in their normal application.

PDFs use installed Poppler `pdfinfo` and `pdftoppm` against a private bounded
snapshot, so a later replacement of the source path cannot turn a reader into
a FIFO read. Only the requested page is rasterized, at up to 1,800 pixels.
Page controls and Page Up/Down navigate the document. The native acceptance
fixture checks actual rendered pixels on two different pages.

Text previews display up to 1 MiB, recognize UTF-16 BOMs, and keep incomplete
final UTF-8 sequences out of truncated previews. Common source formats receive
lightweight lexical highlighting for comments, strings, numbers and keywords.
Coloring is bounded to 128 Ki characters and 8,192 spans; character offsets
remain correct for Unicode. Semantic colors come from Canopa's shared palette.
This is a read-only preview, so it does not attempt language-server analysis.

Audio and video use GTK's native media backend and playback controls. They do
not autoplay; changing selection or closing Quick Look pauses and releases the
previous stream. [ADR 0023](0023-preview-control-focus-and-media.md) adds actual
WAV and VP8/Vorbis WebM playback acceptance, controls that receive native input,
source/owner lifetime checks, and the always-visible video transport.
[ADR 0024](0024-preview-opened-media.md) binds prepared media to the worker-opened
regular file. Binding browsing intent before open and broader format/backend
packaging remain required before claiming the entire preview requirement complete.

`scripts/verify-files.py` runs against a temporary HOME, private D-Bus, and nested
Weston/Niri, with no installed desktop activation. Both appearance schemes and
fractional scaling are in the desktop acceptance inventory. Synthetic input
waits for compositor focus/configure round trips before the next action. Tests
retain screenshots and results; they never mutate real user folders.
