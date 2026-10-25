# Reviewed disposal retention

Status: scoped schema47 verification. Continues
ADR0137. The complete five-area Files goal remains active.

An ambiguous disposal must be resolvable through Files without moving private
files externally. Keep in Recovery now retains both original work namespaces in
one metadata transaction. It moves and deletes no filesystem entries. Both whole
work directories are exposed for Recovery Copy, including unexpected children;
available leaf records remain catalogued so existing selections retain identity.
The records are protected from expiry and do not grant transfer execution rights.

A fresh, nonserialized review binds the exact lane, pending body, independent
preparation binding and temporary-access context. It observes both work roots and
payload versions through held directories, requiring the original parent and root
generations. Keep repeats those checks inside the metadata transaction and verifies
the original accepted task and history-file generation. Changed observations require
a new review; substituted roots or parents are never adopted. Ordinary transfers
cannot consume these containers; only explicit Recovery Copy may read them whole.

The transaction inserts retained records and verifies their namespace and physical
ownership indexes before clearing the exact journal, binding and context. It verifies
the journal retirement before committing. Any failed or ignored write rolls back
all retained metadata and leaves the original journal/binding intact. Temporary
folder-access records remain independently recoverable; Keep does not discard them
or mint permission-restoration authority.

The public worker captures reconciliation ownership for Keep as well as Reconcile.
After successful retention, continuation is derived from the accepted request and
independent traversal cursor. Retention rows and serialized display items cannot
reconstruct an execution owner. The request still stops for explicit Continue and
refuses changed sources, destinations or devices through its original observations.

A private fault test reproduced a false Finished result twice when SQLite ignored
journal deletion: the journal remained but its binding disappeared. The cause was
assuming successful SQL execution meant retirement happened. The transaction now
verifies its postconditions. Public restart tests cover both-root preservation,
whole-root copies with raw Unix names, stale/deserialized review, root substitution,
and aborted/ignored retention, indexing and retirement writes.

Validation on 878 unchanged build inputs passes 500 selected backend tests across
20 targets (42 ignored helpers), both required active/stopped batch restart gates,
and 266 normal isolated desktop-feature library tests (22 ignored). All runs have
zero failures/filters. Default all-target Clippy with warnings denied, workspace
formatting and fixture schema/syntax checks pass. The exact source and attempt
relations are recorded in `.scratch/file-manager/disposal-keep-verification-01.json`.
Final source plus attempt manifests/logs are archived separately; failed private
roots retain 43 decoded verified entries. The existing Files and Niri executables
and the rebuilt required restart gate are preserved.

Native presentation, restricted-parent compensation, broader Undo and cleanup,
Trash/Restore, backend separation, product/device/accessibility acceptance,
reliability/performance, complete workspace/native checks and release integration
remain separate requirements. This decision does not complete Copy/Move recovery
or the full goal.
