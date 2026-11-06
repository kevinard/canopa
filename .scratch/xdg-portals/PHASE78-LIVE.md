# Phase78: normal-service new-file and unconfirmed-staging crash acceptance

Full eighteen-interface goal ACTIVE: eleven packaged, six experimental, Print
opt-in. User priority remains Print features first; automatic Qt integration is
deferred. Previous Phase77 was progress: actual reviewed-replacement crash and
Iced recovery evidence is sealed. This phase adds the remaining recorded local
new-file and staging boundaries. All native/tool handles are terminal, with no
pending question, approval, build, agent, commit or PR.

Read [the evidence index](print/publication-stage-evidence.json) and
[source-only replay](print/publication-stage-replay/README.md).

## Actual native results

Twenty runs / sixty groups pass, using unchanged normal470 opt-in backend,
standard frontend277568, real Flatpak identity/filtered bus and actual Iced
file/overwrite consent. No transaction harness or substituted publisher. The
test-only tracer reads actual worker syscalls without changing their arguments,
requests, replies, memory or journal. Only the main service is killed; its real
parent-death contract retires the stopped publisher. Every public pending Print
returns response2 and every recorded producer exit is observed/reaped.

Eight new-file runs cover completed stage, Publishing before link, successful
link before Published record, and synced Published record in dark100/light125.
Discovery reports Interrupted, Result unconfirmed, Result unconfirmed and
Previously published respectively. Normal startup Iced agrees. New unrelated
files created at absent destinations before restart retain their bytes/inodes.
Confirmed bodies export through fresh Iced picker destinations: eight exact
2435-byte PDFs,32 A4 pages with the original labels. Retained work and destination
contents remain intact. Source-identical prior raster evidence is reused.

Twelve additional runs cover new/existing destinations in both appearances:

| Held syscall boundary | Staged bytes | Completion evidence | Restart behavior |
| --- | --- | --- | --- |
| Initial Ready record fsync during setup | 0 | No payload marker; catalog can still contain intent only | Incomplete copy; no body export |
| First actual staging pwrite64 returns65536 | 65536 of80516 | Ready, payload null, no completion marker | Incomplete copy; no body export |
| Complete staged-document fsync returns0 | All2435 known PDF bytes | Ready, payload null, no completion marker | Incomplete copy; no body export |

The partial case compares the actual stored bytes to the exact source prefix,
not an invented worker reply or truncated input. It is genuinely partial. The
full-byte case shows that even a valid complete PDF cannot replace required
completion evidence. Actual disabled document-copy controls are clicked; no copy
or destination picker is offered. Original hardlinks, retained objects and newer
unrelated destination occupants survive. Dark/light Incomplete-copy and new-file
Published surfaces were visually inspected.

The first partial-replace-dark-01 passed three groups. Matrix01 continued only the
remaining nineteen cases, without repeating it. Artifact checks pass all60groups
and all32 delivered PDF pages. Native host audit confirms all570 recorded
PID/start identities absent and all twenty private desktop paths removed. No
failed native acceptance run in this phase.

## Larger input and bounded storage

No C client or Rust rebuild: A81BE remains28560bytes and uses its existing unlinked
public FD path. The larger PDF inserts comment lines before the existing xref and
updates startxref; existing objects/page streams stay unchanged. System qpdf
validates the resulting four-page80516-byte document, SHA
`2029ca7b1c16806fd7869c49974baf4ae9c385d0b72fa193dfb2fb960146d91d`.

An explicit private `org.canopa.PortalPrintOther//crash` branch uses that input
and the unchanged136-file SDK. Original Owner/Other branch1, GIO/GTK apps and
runtime were not updated. The first export failed before creating a commit
because the newly assembled directory was not finalized. Receipt01 remains
failed; finish-partial.py finalized the same stage, exported/installed once and
verified commit6eba531e. No SDK transition or source generation was repeated.

After every native process exited, only the explicit crash branch was uninstalled
from the private workspace installation. Original branches and their exact small
documents remain. The verified larger source and staging now share one raw inode;
the local exported commit remains recoverable. No system/user desktop installation,
service, queue or physical printer changed.

To maintain the existing128MiB shared-store cap, only one unused extracted private
developer XML file, Flatpak-1.0.gir (458779bytes), was retired after matching every
byte to the retained signature-verified original package. System SDK data, other
GIR files, runtime libraries and notices remain. Phase78 evidence stays below
16MiB, combined Phase74–78 unique file allocation below160MiB, with a3GiB free
floor. No new executable, SDK, dependency graph, release build, cache purge or
per-case binary copy. The new larger data input is80KiB, not a platform download.

The final guard compares all1358 root inputs against Phase77; root changes are
only fixture README/Print guide. Rust/Cargo/vendor, normal470, unit9cce9bee,
integration0444f8ea and CUPS AD09/AEF779/D3b6 remain unchanged. Tooling/docs/native
validation is appropriate; no workspace gate was repeated. Phase73's exact ten
Files failures still prevent full workspace acceptance.

Next: [cross-restart resources and actual pending worker exit](issues/11-print-cross-restart-resources.md).
Current per-Client semaphores/source facts are an investigation lead, not a
reproduced leak. Verify the actual native kernel obstruction and existing service/
compositor/worker limits before designing a correction; do not call a ReapGate or
ptrace stop an uninterruptible wait. Then continue Documents/standard-runtime,
device/driverless/custom precision, deployment/release/routing and six experimental
gates. Power-loss durability, physical printers and full goal completion remain
unproven. Do not rerun completed crash/token matrices, SDK setup or workspace
checks solely because a new goal turn starts.
