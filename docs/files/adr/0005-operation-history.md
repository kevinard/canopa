# Persistent operation history and atomic name changes

Status: accepted under delegated implementation authority, 2026-10-16.

Undo belongs to the Files application, shared across its Browser Windows and
Tabs. It survives process restart. The default history contains the latest 100
completed operations, so retained replacement data must remain available until
its associated entry expires or is undone. Implementing retention for transfers
is still required; the first executable slice supports folder creation and
rename only.

The initial history lived in private, bounded, atomically replaced JSON state.
[ADR 0006](0006-transfer-transactions.md) supersedes its storage format with
incremental SQLite history and migration of `operations.json`. Unix paths retain their original
bytes. One advisory file lock prevents two engines from mutating the same
history. Native application initialization opens it only in the primary
GApplication process. Directory reads, state writes and operations run on
workers; a native queue preserves the user's command order and holds the
application alive while queued work is running.

[ADR 0016](0016-undo-namespace-index.md) adds schema version 2's transactional
namespace indexes and migration for bounded Undo relocation work.

Each atomic filesystem change first writes and syncs its intent. Folder
creation prepares an owned hidden stage next to its destination, then commits
it with Linux `renameat2(RENAME_NOREPLACE)`. Rename and its reversal use the same
no-replace operation. An unavailable or unsupported atomic primitive reports a
failure instead of falling back to overwriting a destination.

Startup recovery reconciles incomplete intents with actual filesystem object
identities. It registers changes that committed, retains uncompleted Undo
entries, and cleans owned, empty staging directories. It does not replay a
destructive action at startup. Crash-recovery stress tests remain required
before extending this seam to transfers and retained replacements.

Undo of creation refuses a folder containing newer files. Undo of rename
refuses an occupied original path or a replaced source. Failed reversals retain
their history entry. The current interface reports these conflicts; the full
transfer/Undo conflict-choice interface remains unfinished. Filesystem identity
checks and transfer fingerprints must be audited together as copy, move and
replacement operations are added.

Ctrl+Shift+N and the overflow menu create a folder; F2 and the menu rename the
focused selected item. The name prompt accepts one basename and does not
reinterpret an unchanged lossy display name as a new path. Ctrl+Z and a visible
action identify and undo the latest operation. Text fields keep their own
editing Undo. Successful name changes reveal and select the resulting item in
the originating tab when it is still browsing that folder.

Real temporary-filesystem tests cover raw filenames and restart, existing
destinations, newer contents, cancellation before mutation, unavailable journal
storage, and exclusive ownership. Native acceptance exercises the actual name
prompts and two successive reversals. Batch copy/move, Trash, conflicts,
replacement retention, progress/cancellation/retry, notification actions and
the Operations window are still required by the product brief.
