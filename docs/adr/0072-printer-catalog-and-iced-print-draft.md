# Printer catalog and Iced print draft

Status: private catalog/draft foundation scoped verified; full Print unfinished.

Print needs user-selected destinations and device options before the prepared
authority from ADR0070 can be issued. The document/CUPS worker from ADR0071 has
scoped submission acceptance, but that does not provide printer discovery or a
print dialog.

The same isolated Rust worker now exposes bounded Printers and Capabilities
operations through the public CUPS destination API. A catalog preserves queue and
saved-instance identities, status, accepting/default flags and labels. Capabilities
carry advertised copies, fixed media geometry/margins, sides, color modes and
formats. Missing attributes remain missing. Standard saved media/sides/color
preferences are usable only when the current advertised values support them.
The worker client verifies the returned destination identity and bounds before
letting metadata reach the presentation model. Submission still independently
revalidates its explicit options.

A native empty-scheduler probe exposed CUPS' retained not-found status when no
queues are listed. Only a zero-count catalog treats `0x0406` as empty; submission
still requires the IPP success class. The public implementation preserves that
last status when the destination count is zero. See the primary
[CUPS destination implementation](https://raw.githubusercontent.com/OpenPrinting/cups/v2.4.20/cups/dest.c).

The hidden `--print-dialog` hosts one Iced event loop and one retained Tokio
runtime for metadata child processes. Switching destinations aborts the old query;
the existing client-owned task still retires its helper. A replacement query waits
for those operation slots to drain. Generation checks keep late results and stale
approval actions from changing the current draft. Closing the event loop signals
cancellation, closes client admission and waits for helper retirement before
stopping Tokio. An idle check alone would allow a queued, not-yet-aborted query
to start after the drain; closed admission rejects it before spawning. Initial Enter
does not approve. Only the explicit current approval action can return a Grant.

The draft separates layout from device options and the target. It exposes page
modes/ranges, copies/collation/order, scale, orientation, page set and n-up;
advertised printer paper/sides/color; and file format/paper geometry/margins.
Display ranges start at one, while returned layout ranges start at zero. No layout
transformation is performed here. The future wire adapter must resolve which
transformations a prepared client already performed before submitting device
options; returning the same copies twice is not an accepted integration strategy.

Print to file embeds the existing Files Save picker in the same window. Formats
are restricted to the caller-supported set; direct Print additionally requires an
identified matching input format, so changing an extension cannot promise
conversion. Changing the format retires the prior path selection. Paths retain
original Unix bytes. Neither the picker nor this draft creates/truncates output.
The parent still needs pinned target ownership and replacement/race validation
before issuing a prepared token or publishing bytes. This private path result is
not yet sufficient evidence of final file-output authority.

Explicit file geometry uses a custom paper identity and clears old PPD aliases.
This follows the consumer requirement that a variant have dimensions plus a paper
identity; otherwise deserialization can return no paper. See the primary
[GTK paper-size implementation](https://raw.githubusercontent.com/GNOME/gtk/gtk-3-24/gtk/gtkpapersize.c).
GTK remains a consumer compatibility reference, not a backend UI dependency.

The final empty-catalog/admission-corrected source passes formatting, strict
original-workspace/all-target Clippy, all-target compilation,46 Print tests and
complete Portal regressions (261 library/8 ignored,55 integration/2 ignored).
Twenty-four scoped native groups pass in dark/100% and light/125%, including
catalog/geometry/saved instances, approval/concurrent drafts, actual blocked-query
retirement, and same-window Files picker selection/cancellation without output
creation. Sixteen Iced dialogs and61 observed processes are terminal. The normal
binary has neither GTK linkage nor a startup CUPS dependency. Receipts are in
`.scratch/xdg-portals/print/dialog-evidence.json` and `target/portal-print-31/`.
This does not establish public Print or complete UI/control acceptance. Native
images expose scroll-content right-edge clipping; the n-up picker also needs all
valid initial values represented. Remaining work includes full printer/custom-media capabilities,
legacy saved-instance aliases, target pinning, exact Print wire/lifetime/Lockdown,
backend death/ambiguous creation, actual toolkit rendering/copy compatibility,
file replacement races, parent/modality and both-appearance native acceptance.
The descriptor remains unchanged and Print is unadvertised.
