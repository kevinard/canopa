# Iced source-bound floating Quick Look

Status: accepted under delegated implementation authority, 2026-10-18.
Text/source/image/PDF implementation passes scoped native acceptance. Media and
the complete preview acceptance matrix remain in progress (see ADR0089).

Keep one Quick Look per originating Browser Window. Follow that window's active
Tab and selected focused file, falling back to its first selected file. Another
Browser Window's focus and selection cannot retarget it. Closing its source
closes the preview and cancels preparation. Closing the preview retains the
source browser, selection and scroll. Search results use this same selection
source, including their original Unix path bytes.

Use the shared native transient operation from root ADR0019, with a requested
760 × 600 size and a 320 × 240 minimum. The native parent is set before initial
configuration. The compositor chooses actual placement and focus. A failed
native opening is visible on the source browser; it does not
leave an unrelated tiled preview. No application-specific Niri rule is required.

One reader thread and one replaceable pending request belong to each preview.
Selection/navigation changes cancel previous work. Window identity and a request
revision guard preparation results, text actions, viewport reports and controls.
Reset content and control focus when the selection changes. Keep the previous PDF
page visible only while the next page prepares; a failed page displays its error.
Reuse the existing bounded image/PDF/text/media preparation core.

Default preview focus continues source browsing. Space/Escape closes the preview;
Tab and Shift+Tab cycle available actions, with a shared visible focus treatment.
Space/Return activates a focused action. Clicking text allows native text
selection/copy/navigation, while edit actions are ignored. Tab leaves text for
the action controls. PDF Page Up/Down changes pages; source navigation continues
through the originating window at every browser width.

Images and PDFs start fitted to the content viewport. Zoom steps start from the
actual fitted scale; Fit restores adaptation and 100% uses the bitmap's logical
size. Fit can exceed manual zoom bounds; disable unavailable zoom directions
using the actual current scale so Zoom in cannot shrink or Zoom out enlarge.
Keyboard traversal skips unavailable controls. Information identifies the
prepared file and bounded/truncated text.
Prepared syntax colors have scoped verification under ADR0089. Full audio/video
controls remain part of the migration and must be verified before declaring
Quick Look complete. The complete product
brief, accessibility, source/media authority and native matrix remain required.

The original checkpoint passed nineteen adapter tests and ten public preview
tests. Native runs 20/21
(dark at 100%, light at 125%) each pass 15 checks, including actual text copy,
read-only editing, keyboard control activation, rendered image zoom/Fit, PDF
pages, source focus and search/tab/window ownership. All 22 captures were
inspected. See the [checkpoint](../../../.scratch/file-manager/ICED-PREVIEW-WIP.md)
for the exact executable, retained diagnosis evidence and remaining gates.

[ADR0089](0089-iced-prepared-source-highlighting.md) adds worker-prepared source
highlighting, palette invalidation and explicit LF text-copy semantics. Its
checkpoint passes 56 Iced library tests and 17 native preview groups in each
theme, plus the same-executable large-text Recovery regression. These scoped
results do not close the media, accessibility or full preview gates.
