# Print physical copy boundaries

Status: accepted; scoped native copy/duplex verification; Print unregistered

## Context

The [public Print contract](https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.portal.Print.xml)
asks an application to format its document using the returned PreparePrint
settings, then deliver that document with the retained token. The controller must
distinguish rendering instructions from the complete approved printer job.

An actual GTK3 probe produces six PDF pages for a three-page document with two
copies, under both simplex and duplex settings. It inserts no blank back between
odd-page copies. Uncollated copies repeat individual PDF pages, which cannot
preserve front/back pairs when the printer subsequently applies duplex. The
same probe produces three pages when returned `n-copies=1`.

## Decision

For printer destinations, return `n-copies=1` to the application while retaining
the complete approved copy count and collation in the server's one-use approval.
All other approved application layout settings still describe one formatted copy.
For prepared printer documents, the backend applies only physical copies; it
does not apply ranges, imposition, scale, margins or orientation again. Each
prepared page keeps its own normalized geometry. Copy-only requests reject
additional layout settings rather than silently discarding them.

Direct printer documents apply layout and physical copies in the backend.
Collated copies repeat the complete side sequence. Uncollated duplex copies
repeat front/back pairs. Multiple copies of an odd-side document get a blank
back after the last front of each copy, so the next copy begins on a new physical
sheet. A single copy can end on an unpaired front. The same rule applies after
number-up imposition. Both duplex edges use the same page grouping; the approved
side setting tells the printer how to turn the sheet.

The printer receives `copies=1`, since physical copies are already in the body.
For multiple printer copies, consent retains either an explicit side setting or
the reported default; an unknown device default cannot determine copy grouping.
Print-to-file keeps the original application copy count and prepared bytes, as
there is no physical sheet boundary to implement.

## Validation scope

Formatting, strict workspace/all-target Clippy and the original workspace's
all-target compilation pass. The compiled harness passes 94 Print tests,
12 Lockdown tests, all 312 Portal library tests and 55 integration tests. The
14-group actual isolated-worker copy matrix passes with all 52 observed
processes terminal. It checks collated/uncollated simplex and duplex sequences,
single copies, imposition before copying, blank backs, mixed page dimensions,
crop/rotation/PDF units, and refusal of additional copy-only layout settings.
The existing 24-group production file/conversion/storage/lifetime matrix also
passes, with all 123 observed processes terminal. No cleanup kills were needed.

The public canonical frontend/controller/Production/Iced/private-CUPS matrix
passes 36 groups across dark/100% and light/125%. Twenty-two actual loopback
printer deliveries verify scheduler copies=1, the requested side mode, exact
collated/uncollated page order with blank backs, and application page geometry.
These include four live GTK printer groups, four explicit Qt DBus/PDF groups
and four direct Print printer groups. Prepared print-to-file still returns the
approved copy count and preserves its already-formatted body. All 172 observed
public fixture processes are terminal; the passed matrices together account for
347 observed terminal processes, with no cleanup kills.

The authoritative receipt is `target/portal-print-37/acceptance-01.json`, with
the concise index in `.scratch/xdg-portals/print/copy-evidence.json`.
Normal executable, harness and guarded source hashes match. Startup linkage
does not include GTK, CUPS or qpdf. The original build graph/cache is reused;
phase net storage growth is about 89 MB. No new Rust native driver or executable
copies were added; the small GTK/Qt clients are stored once by checksum.

Retained failed runs exposed fixture assumptions: the PPD Duplex default also
had to change alongside sides-default, Unix socket paths needed shortening,
and Qt's actual 595×842-point A4 output had to be compared with its own geometry.
An expanded driver was interrupted after 13 groups; its survivors were retired
using recorded identities and exact private runtime/queue environments, and a
final host check found none. The successful driver uses shorter bounded segments.
These failures are not counted as full passes. Rust inputs stayed unchanged.

The earlier GTK rendering probe is recorded in
`target/portal-print-gtk-render-05/summary.json`. It is an application-side
measurement with a controlled frontend, not backend acceptance.

The Qt probe explicitly uses Qt DBus and QPrinter PDF rendering. Its result must
not be presented as automatic QPrintDialog portal integration: the
[Qt portal platform theme](https://github.com/qt/qtbase/blob/dev/src/plugins/platformthemes/xdgdesktopportal/qxdgdesktopportaltheme.cpp)
handles FileChooser and delegates other native dialog types to its base theme.

Physical printer behavior, broader reverse/page-set/duplex layout combinations,
toolkit/device settings, prepared PS/SVG, real Flatpak identity, file-publication
startup recovery and ambiguous CUPS submission remain separate gates. The ten
recorded Files integration failures remain unresolved; no new full-workspace
test gate ran. This decision does not register or advertise Print, alter installed
queues, or promote the six experimental portal interfaces.
