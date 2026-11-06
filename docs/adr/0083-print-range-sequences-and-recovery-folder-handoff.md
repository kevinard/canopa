# Print range sequences and recovery folder handoff

Status: scoped implementation and native acceptance passed. Print remains private,
unregistered and unadvertised. This does not promote the experimental interfaces
or close the recorded Files/workspace failures.

A direct print request must preserve the approved range sequence, including
overlapping pages. On a seven-page document, the displayed ranges `5-7, 2-5`
produce pages `5, 6, 7, 2, 3, 4, 5`. The former Rust renderer instead selected a
sorted union, producing `2, 3, 4, 5, 6, 7`. That lost both order and a repeated page.
An actual GTK3 client through a private measurement frontend reproduced the
sequence; the normal Rust worker reproduced the mismatch. The previous unit test
asserting merged ranges was an incorrect compatibility assumption.

The renderer now concatenates each zero-based range in declared order, clipping
it to the actual document. A range wholly beyond the document contributes no
pages. Selection precedes n-up imposition; odd/even output-side selection and
reversal follow imposition, then copies preserve the established physical duplex
boundaries. Existing request/range/input-page/output-side limits still apply.
Prepared documents keep their provenance and are not subjected to layout again.
GTK's upstream implementation provides the corresponding client-side sequence
construction and sheet selection in
[gtkprintoperation.c](https://raw.githubusercontent.com/GNOME/gtk/gtk-3-24/gtk/gtkprintoperation.c).
No GTK implementation or dependency is added to Canopa.

Regular regressions cover repeated pages, range order combined with imposition,
page-set/reverse/duplex copies, document clipping and output-side limits. Native
comparison also checks actual output PDF page membership/order, physical paper
dimensions, the eight four-up corner orders, and collated/uncollated duplex backs.
The GTK measurement frontend is a client compatibility probe; it does not prove
the Canopa public controller, Iced consent, CUPS delivery or physical printing.

Recovery's Open folder action uses FileManager1 and an exact file URI. Native
acceptance uses the real resident Canopa Files on a private session bus and
compositor, with owned retained directories containing spaces, `#`, `%`, Unicode
and a non-UTF8 byte. The browser must show the exact retained directory and its
five expected entries. Removing the private Files service must show a recovery
error while leaving both retained bodies and the catalog entry intact. An error
also exposed text clipping when the recovery scrollbar appeared. Both scrollable
columns fill their available width, and embedded scrollbars reserve layout space
so content can wrap without being covered by the scroller. Column width alone
does not change Iced's default floating scrollbar behavior.

Final acceptance is `target/portal-print-42/acceptance-01.json` and
`.scratch/xdg-portals/print/layout-sequence-evidence.json`. Formatting, strict
workspace/all-target Clippy and original workspace/default-feature/all-target
incremental compilation pass. Regular tests pass134 Print/12 Lockdown and the
complete Portal352 library/55 integration suite (19/2 ignored in the full suites).
There is no new complete workspace test execution; the ten recorded Files
integration failures remain unresolved.

The final executable passes36 normal-worker layout/copy groups against22 measured
GTK prepared references, including8 actual PDF four-up cell maps and physical
paper dimensions. Unchanged reference PDFs and the client/compositor are reused
by checksum. Dark100 and light125 recovery flows open the exact retained folder
in real Canopa Files, preserve all5 file hashes and the exact catalog hash, show
an error after the private Files service exits, and scroll to both folder/list
actions without covered text. All189 observed processes in the final accepted
checks are terminal; guarded source/binary pins match. No emergency fixture
cleanup kills were needed. The phase ends with about369MB less disk used, with
the6GB reserve preserved, no executable copies and no new native executable.

Prepared-format acceptance also uses the actual GTK client: a19,864-byte14-page
PostScript body preserves reverse/ordered/repeated ranges and two collated copies;
a16,331-byte single-side SVG contains4 imposed logical pages. The existing native
harness supplies controlled retained approvals to the real Production adapter.
For each format, new output and replacement preserve the body byte-for-byte;
a declared PDF destination refuses mismatched input and preserves its previous
bytes. PostScript round-trip labels and both rendered first-page/SVG images were
reviewed. This does not establish public frontend/Iced/token, printer delivery or
real Flatpak behavior for PS/SVG.

The earlier comparison exposed the range bug (layout01,32 groups/one mismatch).
The first column-width correction passed tests and folder semantics but did not
reserve scrollbar space (folder-dark-03); its receipt records that limitation.
The final embedded-scrollbar correction passes both appearances. Interrupted
folder-dark-01 is not acceptance. Original failures and exact fixture sources
are retained separately; all current tool handles are terminal.

Next gates include public/toolkit/printer prepared-format acceptance, advanced
custom-media/device/saved-instance controls, full backend/crash/stalled-filesystem
and cross-restart resource acceptance, ambiguous CUPS submission, physical/real
Flatpak, runtime packaging/Print advertisement, the six experimental portals and
final workspace/native/package/release routing. All services, documents and
printer state in these fixtures are private. Installed routing and services are
unchanged.
