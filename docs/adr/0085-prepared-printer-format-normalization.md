# Prepared printer format normalization

Status: scoped native acceptance passed. Print remains explicit opt-in and absent from default
packaged routing. Other Print/device/resource and six experimental-interface
acceptance gates remain open.

Phase71 supersedes the single-copy PDF pass-through described below: recognized
prepared PDF printer streams also use copy-only normalization to preserve printed
annotation appearances. Original exact PDF physical geometry is retained. The
single-copy file-output branch remains byte-preserving. See
[the subsequent decision](0107-exact-paper-and-print-appearances.md); the native
results below retain their original phase43 scope.

Prepared PostScript and SVG printer jobs must work with one approved copy as well
as multiple copies. The former Production path normalized prepared bodies only
when physical copy expansion was required. With one copy it submitted the original
PS/SVG MIME type directly to CUPS. A private PDF-only queue rejected those jobs at
Send-Document with `client-error-document-format-not-supported`. The same bodies
succeeded with two copies because copy expansion also normalized them to PDF.

The actual normal backend, standard frontend, Iced printer consent, private CUPS
and loopback printer reproduce four failures in a15-group baseline: one-copy PS
and SVG in simplex and long-edge duplex. The PDF control and all multi-copy
variants pass. PS input is a three-page PostScript conversion of the existing
GTK-prepared PDF; SVG input is the actual GTK-prepared single side containing
four logical pages. These are prepared-stream protocol tests, not live GTK
printer emission or physical-printer acceptance.

Production now sends recognized prepared PS/SVG printer bodies through the
existing copy-only renderer even for one copy. That step normalizes the format
and preserves the already formatted physical sides; it does not apply the saved
logical ranges, reverse, scale, n-up or paper setup again. The approved copy count,
collation and duplex boundaries still control physical copy expansion. Single-copy
prepared PDF keeps its existing pass-through path, and prepared file output keeps
exact source bytes. Unknown-format behavior is unchanged.

The renderer and its parsers retain the existing namespaces, source/derived-body
storage permits, cancellation, size/time limits and retirement ownership. The
printer submission still requests one scheduler copy after body preparation.
No new parser, Cargo dependency, native executable or GTK implementation is added.

A regular adapter regression records the first worker operation and intentionally
stops before parser or printer I/O. Despite nondefault approved logical layout,
one-copy prepared PS/SVG must request PDF copy-only rendering with default logical
layout/setup; prepared PDF must submit directly. The native matrix then checks
actual received PDF content, physical dimensions, raster content/blank backs,
one completed scheduler copy, both duplex edges and collation through real
public consent. Formatting, strict workspace/all-target Clippy and the original
workspace/default-feature build pass. All135 Print/12 Lockdown and complete Portal
353 library/55 integration tests pass. No new full workspace execution is claimed;
the ten recorded Files integration failures remain open.

The final public printer matrix passes15 cases per appearance (dark100/light125),
and12 public file-output checks pass again on the same binary. All201 observed
processes are terminal. The source/binary guards match, with no executable copies,
new native driver or dependency changes. Phase net storage growth is94,494,720
bytes, leaving7,219,273,728 free at the seal. See the durable
[evidence](../../.scratch/xdg-portals/print/prepared-printer-evidence.json) and
[active checkpoint](../../.scratch/xdg-portals/CONTINUE.md). These results cover
prepared-stream printer delivery; automatic Qt dialogs, live toolkit printer
emission, advanced device controls, publication/CUPS crash and ambiguity, wider
resource acceptance, real Flatpak/physical printing and runtime packaging remain.

Geometry is compared with the input format's intrinsic page dimensions. The GTK
SVG fixture declares unitless width/height, so its independent rsvg conversion is
446.457×631.417 points, not the A4 paper selected in the job. Copy-only preparation
preserves that extent, just as it preserves differing prepared PDF page sizes.
Changing it to A4 would apply another paper transformation. This follows SVG's
initial coordinate system and the CSS pixel/physical-unit relationship:
[SVG coordinate systems](https://www.w3.org/TR/SVG/coords.html),
[CSS absolute lengths](https://www.w3.org/TR/css-values/#absolute-lengths).
The first strengthened fixture incorrectly hardcoded A4; its mismatch is retained
separately and does not establish a renderer defect.

The controlled PS fixture explicitly encodes595.28×841.89-point page boxes. The
pipeline preserves them; a standalone host Ghostscript reference rounded to
595×842 points because of its conversion defaults. The final assertion reads the
source's explicit page boxes. Earlier reference-only mismatches are retained as
failed receipts, separate from both the reproduced MIME rejection and final
acceptance. Actual received PDF raster/content, copy/blank-side order and intrinsic
geometry pass without renderer or parser changes.
