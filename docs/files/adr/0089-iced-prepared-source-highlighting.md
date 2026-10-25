# 0089: Iced renders prepared source highlighting

Status: prepared highlighting and scoped native previews verified, 2026-10-19. Continues
ADR0059/0074/0088. The complete preview and product gates remain open.

The preview backend already recognizes supported source filenames and prepares
bounded lexical spans. The Iced editor ignored those spans. A native fixture
opens an actual Unicode Rust file with CRLF endings and observes ordinary text
where string, number and comment colors are required. Its first run fails on
those pixels while the source preview is mapped and floating.

Keep lexing in the existing backend. The preview worker also converts its
absolute character offsets to per-line UTF-8 byte ranges for Iced, using the
renderer line iterator so paired line endings remain consistent with the editor.
Stop at the last prepared highlight; ordinary text needs no line catalog. The
immutable adapter result travels with the prepared preview under the same
source/window/request revision guard. Neither view construction nor highlighting
callbacks perform filesystem reads or run the lexer.

Match each incoming editor line against its prepared text before applying
ranges. This also handles the initial empty editor required by ADR0088. A new
selection gets new prepared metadata, while stale results cannot style its text.
Keep the existing read-only selection/copy behavior and bounded text insertion.

Use bold keywords, Success for strings, Warning for numbers, and italic Muted
comments, following the existing lexical categories and shared Canopa palette.
Fall back to the normal text foreground when a syntax role lacks 4.5 contrast
against the editor background or its Raised selection surface. Theme colors
participate in highlighter settings identity so palette changes restyle cached
content. No additional parser, syntax dependency, language grammar or GUI toolkit
is introduced. The existing lightweight lexer and its coverage limits remain.

Verify Unicode offsets, paired endings, multiline spans, cancellation, unchanged
text, stale/empty editor lines and live palette changes through the actual
preview/highlighter interface. Native acceptance must inspect rendered colors,
raw Unicode clipboard bytes and return to plain text, retain image/PDF/source ownership
coverage in both themes, and preserve the large-text Recovery workflow. This
does not establish media playback, complete language parsing or full performance
and accessibility acceptance.

Native copy uses the editor's text convention: multiline selections contain LF
separators. Preparation and the read-only buffer retain source line endings;
copying does not alter the file. The first native assertion incorrectly expected
CRLF on the clipboard. A raw-byte probe confirms that all Unicode and text match,
with exactly the seven CRLF separators converted to LF (127 source bytes become
120 clipboard bytes). Keep this normal editor behavior and compare exact UTF-8
clipboard bytes against explicit LF text. Retain both original and copied bytes
as private fixture artifacts; do not use subprocess newline normalization.

Verification passes all 56 Iced library tests, including four new tests using
real backend-prepared source, default all-target Clippy with warnings denied,
formatting and the default application build. The same executable passes 17
native preview checks in each theme (dark100/light125), including rendered
lexical colors, exact Unicode clipboard text, live palette changes, plain-text
reset and the existing image/PDF/source-ownership workflow. The large-text
interrupted Recovery regression also passes five groups in each theme. Its first
combined runner received an unexplained SIGTERM after dark passed; the incomplete
light attempt is excluded. Exact private process cleanup precedes a fresh passing
light run. No application change or diagnostic delay separates those runs.

A separate startup investigation passed 16 fresh sessions on the preceding
executable under the original readiness deadline, including the large interrupted
fixture setup. It reproduces no startup failure and establishes no repair or
production performance claim. The suspected missing narrow Refresh label was a
visual misreading: corresponding original and probe pixel regions are identical.
Media playback, broader accessibility/performance, restart authority and complete
product acceptance remain open. See the
[verification receipt](../../../.scratch/file-manager/syntax-verification.json).
