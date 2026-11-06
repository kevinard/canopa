# Automatic Qt Print routing: measured client gap

Status: measured compatibility boundary; automatic Qt portal acceptance remains
open. Print stays opt-in and absent from packaged routing.

The existing Qt compatibility probe explicitly invokes the public Print portal
and renders with QPrinter. It establishes backend interoperability but does not
exercise ordinary QPrintDialog routing. The next acceptance step must distinguish
that probe from the toolkit's own behavior.

A new disposable application calls only public QPrintDialog/QPrinter APIs. It has
no Qt D-Bus code. The private fixture provides a healthy standard frontend and
normal Canopa backend, advertises Print and FileChooser version 4, forces the
installed xdgdesktopportal theme, observes loaded plugins and monitors Print
method calls. CUPS, discovery bus, compositor and PDF-only receiver are all owned
private fixtures; no installed queue or service changes.

On this installed Qt 6.12.0 Wayland configuration, both cancel and print show the
Qt Unix print dialog and produce zero Print portal calls. Cancellation creates no
job. Approval delivers one exact three-page PDF directly through CUPS. Its saved
scheduler control reports completed job 1 with the ordinary Qt document title.
The caller's maps include the Qt PrintSupport and xdgdesktopportal plugins and
contain no GTK library.

The separate explicit Qt portal control opens the actual Canopa Iced surface.
The same bus monitor then observes PreparePrint and Print, the frontend responds
successfully, and the receiver gets the approved two copies as one six-page PDF.
Saved scheduler job 2 has the exact Canopa ownership tag, state 9 and one scheduler
copy. This positive control also verifies the monitor and frontend path.

The upstream v6.12.0
[portal theme](https://raw.githubusercontent.com/qt/qtbase/v6.12.0/src/plugins/platformthemes/xdgdesktopportal/qxdgdesktopportaltheme.cpp)
creates its portal helper for FileDialog and delegates other dialog types to its
base theme. The upstream
[Unix print dialog](https://raw.githubusercontent.com/qt/qtbase/v6.12.0/src/printsupport/dialogs/qprintdialog_unix.cpp)
constructs QUnixPrintWidget and executes the Qt abstract print dialog. The
[CUPS printer support](https://raw.githubusercontent.com/qt/qtbase/v6.12.0/src/plugins/printsupport/cups/qcupsprintersupport.cpp)
creates the native CUPS print engine. These sources explain the measured path;
the installed binaries and runtime observations are separately pinned. This is
not a claim about every Qt version, downstream build or sandbox configuration.

Automatic acceptance requires an actual ordinary Qt caller to reach the portal.
The native result remains `automatic-qt-routing-mismatch`; the two automatic
cases are retained as counterexamples. Canopa backend requests cannot repair a
client path that sends them no request. A maintainable client integration and
its distribution/lifetime contract are tracked in the
[follow-up investigation](../../.scratch/xdg-portals/issues/01-automatic-qt-print-integration.md).
No Qt fork, new backend interface or default routing was introduced here.

The 55,176-byte caller executable was compiled once with strict C++ warnings and
moved into the existing checksum-addressed fixture store. It is not copied into
each artifact directory. The unchanged Portal Rust inputs and executable still
match the phase48 validation receipt; no Cargo graph or workspace tests were
rerun for this tooling-only change. All 24 observed fixture processes are terminal
without emergency cleanup. Measured phase storage growth is 1,933,312 bytes and
the 6 GB free-space reserve remains intact.

See [the evidence](../../.scratch/xdg-portals/print/qt-routing-evidence.json) and
[checkpoint](../../.scratch/xdg-portals/CONTINUE.md). Automatic Qt integration,
advanced printer/device/authentication controls, publication crash/global
resources, real Flatpak/physical/runtime packaging, the six experimental
interfaces and final workspace/release gates remain open.
