# Continue the full Files goal

The full five-area goal remains active. Current schema45 adds independent
preparation and empty-cleanup task bindings, accepted-record preservation and
separate interrupted reports requiring explicit Recovery review. It does not
finish generic disposal or all restart recovery. No subagents, commits, vendor
edits or protected app/Niri rebuilds. Preserve unrelated work and all evidence.

Current source is stage-task-inputs-14.json:875 captured paths. Validation is
terminal and green: 488 backend tests across 20 targets (41ignored), 254 isolated
default-library tests (21ignored), zero filters, both required batch restart gates
unignored, default all-target Clippy -Dwarnings, format and fixture schema/syntax.
Verifier02 terminal0 writes stage-task-verification-01.json with exact input
relations, all failed/green attempts, required gates and protected binary hashes.
Default receipt: continuation-core-1792716069-receipt.json, elapsed396.16s including
compile, private HOME/XDG/private bus, no display/system bus/activation directories.

Failure archive 02 SHA256 is
29c1cb91057aa908806dbfda85c3ca851e9a1309a13074c9ab7e53a961ca2031:
113 retained roots/755 decoded and live-verified entries, including raw symlinks,
1 hardlink and empty directories. Archive01 failed only its regular-file/hardlink
verification assumption and remains preserved. Read STAGE-TASK-WIP.md and ADR0136.
Source archive 01 and source-final-live01 freeze exact source, docs, failed/green
logs and proof. Verify their recorded hashes before further edits; do not rewrite
frozen evidence. If archiving is still running, await exact handle and perform
verify-stage-task-source-live.py after terminal success.

Next required implementation: durable generic Preparing disposal claims. Existing
staging::clean unlinks folder without consumed-name phases. Use actual public
worker crash cuts to reproduce a second crash after unlink, then reintroduce the
same original inode at folder. Further opens must preserve that new namespace.
Keep original parent/work/payload proofs, failed phase-save rollback, unknown and
unregistered data, and Recovery inspection/copyability. Metadata bindings alone
do not solve this. Follow the diagnosis loop and show ranked falsifiable hypotheses
before fixing an actual reproduced failure.

Then finish legacy/reverse Undo and expiry, queued lineage, large reports/paged
owners, Trash/Restore; renderer-independent navigation/search/preview; product,
device/accessibility/physical acceptance; reliability/performance; complete native,
workspace/Shell/Notify/Niri distribution/release integration and GTK retirement.

Run from workspace root with CARGO_INCREMENTAL=0 TMPDIR=/tmp, -j1 --locked --offline.
Disk is tight: only reclaim exact completed owned test ELFs through the audited
reclaim-leaf-test-cache.py. Preserve rlibs, protected binaries, required restart
ELFs, source/failure archives and private failure roots. Never rebuild an ELF while
its old parent/child test is running. Timeout is observation, not terminal exit.
