# Isolated Print layout and production adapter

Status: historical scoped private production-file-adapter and direct-renderer acceptance
passed. Print remains unregistered and unadvertised. This decision does not
promote the six experimental interfaces or close the preceding workspace failures.

`print::Production` supplies the controller's `Backend` port with the retained
Iced presenter and one shared document `Client`. That shared instance bounds the
number of retained source documents and active helpers across requests. Delivery
keeps explicit Prepared/Direct provenance. Prepared bodies pass through unchanged:
the GTK3 evidence in ADR0075 shows that applying copies/layout again duplicates
already rendered work. Direct bodies need layout before file or CUPS delivery.
Both paths submit one CUPS copy after body layout; the other approved device
choices remain on the job. Approval consistency and retained file identity are
checked before work and again at the existing publication boundary.

The direct renderer accepts recognized PDF, PostScript and SVG inputs. An isolated
normalizer converts them to PDF. Rust selects zero-based page ranges, imposes
logical pages into physical output pages, selects odd/even output pages, reverses
output order, and expands collated/uncollated copies. Physical placement includes
page dimensions, printable margins, all supported cell orders, source crop origin,
rotation and units, orientation and percentage scale. Source Form XObjects are
placed by affine matrices and clipped to their cells. The fresh PDF catalog keeps
printable page content/resources rather than source actions and annotations.
PostScript output is converted after layout; SVG requires one output page.

The public qpdf C object API supplies PDF objects and decoded page content. Its
signatures follow the installed `qpdf/qpdf-c.h`; Rust retains the library and
unique document through every call and uses qpdf's matching buffer deallocator.
No private C++ structures are dereferenced. The library is loaded only in the
isolated layout helper. There are no new Cargo dependencies or application-local
vendor copies. Runtime dependencies are bubblewrap, Ghostscript, qpdf,
rsvg-convert and Poppler's pdftocairo. Packaging/availability policy is still a
Print activation gate.

Each converter runs in a new user, mount, PID and network namespace with no
capabilities, a read-only system runtime, private proc/dev, and a bounded tmpfs.
Only the approved sealed source, fresh anonymous output and diagnostic pipe cross
the converter boundary. User home, session bus and destination descriptors do not.
The original Rust layout helper used the same normal backend executable, held
by descriptor and mounted read-only inside the namespace (superseded by direct
FD execution under ADR0077); it starts before Tokio/configuration.
The command becomes namespace PID1 so its death also retires descendants which
change process groups. Parent-death signalling and cancellation/reaping retain
worker ownership; adversarial descendant/backend-death proof is still required.

Each source and derived body is at most128MiB. There are two logical document
slots globally and at most one live derived body per source. The derived body
holds its original storage and the same document permit until its last user or
worker retires. Rendering an already derived body is refused. Intermediate bodies
are sealed anonymous files. Converter address space, CPU, output bytes, temporary
filesystem and diagnostics have independent bounds. Reopening the owned source
FD gives each converter its own cursor; the approved snapshot is never modified.

Further actual GTK probing confirms that page-set and reverse apply after n-up,
then copies repeat physical output pages. The six probes are retained in
`target/portal-print-gtk-render-04/summary.json`. The installed CUPS PDF filter was
also measured, then rejected as our layout engine: it does not implement the same
percentage-scale/options contract. Those probes do not establish the new Rust
renderer, Iced/public controller integration, CUPS delivery or physical printing.

Remaining acceptance includes real output geometry/content, PS/SVG conversion,
retained destination failure behavior, storage permits, cancellation and backend
loss, hostile document isolation, toolkit prepared-rendering compatibility,
duplex/collation boundaries, the actual frontend/Iced/file/CUPS flow and runtime
packaging. Unknown direct document formats remain unsupported. No installed
routing, service or queue is changed by this decision.

Current acceptance uses the normal backend executable and the exact Cargo-selected
library/integration harnesses. Format, strict workspace/all-target Clippy and the
original graph all-target build pass;89 Print and12 Lockdown tests pass. Full
Portal regressions pass307 library/13ignored and55 integration/2ignored. The
native matrix passes24 groups:21 controlled file deliveries/refusals, retained
storage limits, converter cancellation and abrupt owned-worker loss. All120
observed processes are terminal. Prepared bodies retain exact bytes even with
nondefault copies/layout; direct bodies apply the measured page/copy order.
75% scale produces exactly75% text-box width and height. PDF/PS/SVG outputs also
pass scoped round-trip parsing, page checks and visual audits. Cairo may outline
PostScript glyphs, so lack of extractable Unicode is not a blank-page assertion.

This matrix supplies retained approvals directly to the actual Production adapter;
it does not run public frontend/Iced consent or the adapter's printer delivery.
It does not establish full backend death, arbitrary hostile descendants, physical
printing, toolkit prepared PS/SVG or Qt behavior. Duplex/copy boundaries, unknown
direct formats, executable replacement while a worker runs, startup recovery,
filesystem stalls and runtime distribution remain explicit gates. The six
experimental interfaces, final release/routing and ten preceding Files failures
remain open. No new full-workspace test gate ran.

A Lockdown regression exposed acknowledgement before observer retirement. Stop
now drops the policy watch sender before acknowledging shutdown; the existing
regression and complete Portal suite pass. Failed earlier validation/audit
receipts are retained: sandbox socket denial, a native driver JSON field error,
and an incorrect text-extraction expectation for outlined PostScript. Corrected
checks pass with unchanged guarded inputs and executable pins. Receipts are
`target/portal-print-35/acceptance-01.json` and
`.scratch/xdg-portals/print/render-evidence.json`. The same root incremental cache
was reused, with no new Rust native driver or binary copies; net phase storage
growth is negative (about139MB less used at sealing).
