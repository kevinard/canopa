# Native file clipboard work

Status: progress, 2026-10-18. The entire PRD goal stays active; Iced and
`canopa-ui` remain mandatory. This transport slice does not complete the product,
clipboard UI, transfer dialogs, source ownership or durable restart acceptance.

## Current implementation

ADR0069 adds the multi-format clipboard required by existing Copy/Cut/Paste
commands. `file_clipboard::Selection` preserves original Unix path bytes through
GNOME copied-files, URI lists and KDE cut-selection, rejects malformed/remote or
silently normalized locations, and limits payloads to16 MiB.

`file_clipboard::wayland::Clipboard` is a non-widget worker behind the optional
`wayland-clipboard` feature, enabled by desktop. It uses the supported compositor's
ext-data-control protocol on its own named-display connection. Initialization,
reads, writes and publication have deadlines; one descriptor loop also observes
shutdown. The worker never consumes or changes Iced's WAYLAND_SOCKET. Startup
commands get an explicit starting error; terminal requests get a visible error.
Shared ownership is published before Copy/Cut acceptance is acknowledged.

Reads verify the original offer after a compositor sync. Cut updates alter only
the original source's payload or destroy that exact source; they never set a new
selection that could supersede another owner. Foreign tokens cannot update it.
No clipboard URI grants filesystem authority. The Iced GUI does not invoke this
service yet.

## Verification

- Codec-red01, locations-red01 and normalization-red01 retain genuine failures;
  their green checks pass all3 public codec parents.
- Protocol probe8743 terminated0: private Niri26.04 advertises ext-data-control.
- Initial native87002 terminated0; expanded native58711 terminated0 with4 parents.
- Startup-red84457 terminated1: actual accepted silent Wayland socket exposed
  the old indefinite initialization; the child failed after8 seconds.
- Startup-green52367 terminated0. Final green39707 also terminated0 with5 public
  parents and2 separately identified helper definitions. Independent wl-copy/
  wl-paste clients verify raw filenames, all3 formats, partial Cut, superseding
  external/foreign owners, paused peers, request deadlines and oversized input.
  The final lifecycle case checks explicit startup-read errors, accepted-socket
  shutdown while the child remains alive, and preservation of WAYLAND_SOCKET.
- Final native artifacts: `file-clipboard-startup-green-02/`; binary SHA256
  7f2ecc236a7a760315539355ecb8b69b693af689fa70943a3eb23a25843fb6a3.
  The maintained runner builds before starting private HOME/bus/Niri and owns all
  fixture processes. These runs are explicitly terminal; no native fixture lives.
- Immutable input manifests source01 through source05 remain preserved. Source05
  records513 inputs, including the fixture guide and dependency inventory. The
  last change after final native is inventory documentation only.
- Pipeline86974 terminated0: codec3, lean all-target Clippy, default Iced build
  and formatting all pass. Logs and structured results are in
  `file-clipboard-validation-01/`. Binary audit91259 terminated0: version0.1.0,
  115933136 bytes, SHA256
  9df07f9fc7d626e7e8b735b758100768d26f98ef3e5a137d0fb1e36e3555e381,
  resolved shared libraries and no GTK linkage. All513 recorded inputs remain
  unchanged. See `file-clipboard-verification.json` for consolidated proof.
  All current compiler/native handles are explicitly terminal; both reviewers
  are complete. No external blocker. Full workspace checks are still required
  for the complete product and were not rerun for this isolated transport slice.
- Dependency inventory initially failed its stale-lock check (56144); generation
  55533 and the571-dependency verification78851 both terminated0.
  Python syntax, acceptance JSON and tracked whitespace checks pass.

## Standards

The first review reported P2 blocking initialization and P3 repeated MIME
classification. Callback-driven initialization with wake/shutdown and deadlines
resolves P2; Types::flag centralizes the MIME mapping. The follow-up and final
incremental test/docs review report zero unresolved findings. Production code is
unchanged from the independently reviewed source03.

## Spec

Both transport reviews and the final test/docs review report zero findings.
The accepted-connection handshake, startup errors and isolated WAYLAND_SOCKET
preservation match ADR0069. GUI invocation and placement remain unfinished; no
full clipboard or transfer acceptance is claimed.

## Next work

Connect real Iced Copy/Cut/Paste keys, menus and toolbar to the service. Capture
source placement at Copy/Cut and destination Origin before asynchronous Paste,
keep the matching opaque token and application lifetime, then submit through
OperationJobs::submit_transfer. Preserve original source metadata when moving
between windows/tabs or losing the clipboard to a newer owner. Use actual
transfer reports for partial Cut updates, never reconstructed Retry paths.

Add typed transfer conflict/unavailable dialogs, progress, cancellation and
completed-prefix Undo to Iced. Public OperationJobs already exposes these; current
Iced Review handles only Undo questions. Retain original source reservations and
keep restored transfers review-only until exact journal/source/media/descriptor
ownership is proved. Validate actual keyboard/menu workflows in private Niri,
including both themes and narrow/wide tiles. No test hooks in product code.

Preserve every other PRD/ledger requirement: DnD, recovery/devices/media,
Downloads, accessibility/performance, desktop activation and packaging, workspace
checks and final review/commit. Git remains read-only; unrelated shell research
is untouched. Cargo stays offline with one job; source edits, compiler and native
fixtures remain sequential. Root available space was about1.9 GiB before the
last default build; inspect before broad test/release rebuilds. Historical5996a732
native archive and ADR0068 evidence remain separate from current transport proof.
