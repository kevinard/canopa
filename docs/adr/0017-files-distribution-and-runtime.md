# File manager distribution and runtime dependencies

Status: accepted under delegated implementation authority, 2026-10-16.

The GUI/media runtime details and native evidence below describe the former GTK
adapter. [ADR 0018](0018-files-iced-and-shared-controls.md) supersedes that toolkit
choice. The development executable now uses Iced and `canopa-ui`; its text/image/PDF
previews retain the Poppler tool requirement for PDFs. The media worker now links
distribution GStreamer core/app/video libraries directly; the default desktop
feature enables playback while headless `local-runtime` keeps it optional. The
runtime guide and hosted build declarations now include the direct libraries and
development packages. Native media and release acceptance remain open under
[Files ADR0090](../files/adr/0090-renderer-independent-media-playback.md). The independent
application, launcher and updater ownership decisions below still apply.

The remaining GTK runtime and native evidence in this record is historical.

The combined release archive includes Canopa Files as its eighth executable,
alongside its README, both source licenses, runtime guide and standard desktop
entry. Files remains independent of the Shell and Notify user services. The
archive uses distribution GTK, GStreamer, codec and Poppler packages; it does not
ship another system-library or codec copy. Cargo dependency notices remain part
of the existing combined release inventory.

Files needs GTK 4.12 or newer and GTK's GStreamer media backend, which Ubuntu
packages separately from its widget library. The runtime guide declares the
base/good/bad plugin sets, libav decoders and Poppler tools. Hosted desktop/release
build dependencies include that runtime and desktop-file validation tools.
Dedicated native runner guidance includes the encoder, private audio-service,
capture and accessibility requirements. Native decoding is the media gate;
successful linking, configuration validation or --help does not prove it.

`org.canopa.Files.desktop` registers inode/directory with a normal executable
launch and local URI arguments. Its identity matches the GTK application ID.
It does not advertise D-Bus activation without an activation service. Installing
the entry makes Files an available folder handler; choosing it as default remains
an explicit user action. The guide provides per-user installation and registration
commands. Packaging itself changes neither the live session nor MIME defaults.

Native acceptance installs the real desktop entry in private XDG directories,
selects it as the fixture's folder handler, and cold-launches through GIO into a
folder containing spaces and Unicode. The same workflow then decodes common
formats through Quick Look. It can use an extracted archive's executable and
desktop entry. A helper retains the launch process group until cleanup because
GIO itself returns after launching. Actual windows, controls, pixels and PCM
remain the observation seam.

The local companion updater builds and stages Files with the other owning
configuration validators. Its existing binary backup/rollback includes Files.
It replaces the executable without stopping an independently running Files
process; existing windows and transfers continue, and a later process uses the
new binary. The updater does not install initial launcher metadata or rewind
Files operation history. Shell and Notify keep their existing two-service
restart and notification-state backup contract.

The existing updater process fixture now requires all three matching validators,
and all nineteen tests pass. A new case first reproduced missing Files in the
staging directory, then verified replacement, backup and a still-running old
process. The public configuration workflow now checks all three owners,
namespace isolation and private diagnostics. Rebuilding stale local Shell/Notify
binaries was necessary to observe the current shared Files envelope contract.
The expanded release smoke check rejects the old archive for missing Files and
checks a real Files namespace against all packaged validators. Archive assembly
using debug binaries is separate evidence from an optimized hosted release build;
the full release and integration gates remain in the implementation ledger.

Final local assembly and smoke pass in `package-files-final.log` and
`package-files-final-smoke.log`. The archive collects notices for 590 release
dependencies. `package-files-final-evidence.json` records its hash and confirms
all eight extracted executables match their current build artifacts. Native
format/launcher runs 35 and 36 each pass 28 checks in dark and light/1.25×; original
media controls run 37 passes seventeen checks. The three-owner configuration
workflow, nineteen updater tests, runner preflight, inventory and syntax checks
also pass. [Files ADR 0025](../files/adr/0025-media-formats-and-id3.md) records the
tagged-MP3 decoder failure found by this work and the bounded input correction.
FileManager1 was still open at that historical checkpoint. The current Iced
implementation and private cold-activation evidence are recorded in
[ADR0025](0025-files-file-manager-interface.md); complete assembly and
optimized/hosted release verification remain open.

The release documentation now has an explicit source-to-bundle map. Links among
the shipped user guides, application READMEs, configuration and examples are
rewritten for the installed layout. The theme guide and its complete examples
ship together. Development-only references identify their source-checkout path
as text rather than leaving a broken offline link or publishing scratch evidence.
Links in the source guides retain their repository destinations. The release
smoke check checks the
actual extracted guides and rejects missing files and links outside the bundle.
The assembled archive and extracted smoke check verify24 files and39 offline
links, including the notification guide's section anchor. All eight archived
executables match their current build artifacts. `package-docs-final-evidence.json`
records the archive hash and the assembly/smoke logs. This is local assembly using
debug binaries; optimized and hosted release verification remain open.
The smoke check also validates both shipped examples through all three owners,
including their packaged relative Nord theme dependency.
