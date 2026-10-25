# Directory Merge work record

Status: closed for the first slice recorded in ADR0038 and
 directory-merge-verification.json. Full product goal remains active.

All launched sessions are terminal. Final core40482 passes200/13 helpers;
Clippy84610, build68700, native64/48320 and65/2261, and formatting check26419
pass. The directory suite passes10, including7 new Merge parents. Native checks
total34. All10 screenshots were inspected. No native fixture repair was needed.

Final proof includes exact schema9 binary/source hashes, logs, initial failures
and remaining scope. All Rust writes predate final core startup. Earlier core
25530 and73744 failed and are terminal; use core-final-v2.log as final evidence.
The earlier future-schema fixture and invalid-choice error category were fixed.
Reserved-stage Merge had a real red regression before its separate-identity fix.

Only generated Files incremental caches were removed after terminal compilers,
with cleanup JSON records. Source, binaries, evidence and unrelated work remain
intact. Do not poll old handles or restart completed validation from old notes.

Full Merge is NOT complete. IMPLEMENTATION.md Goal continuation records the next
root/remainder review and rescan, cross-filesystem copy/source cleanup, real
process-kill/expiry/Trash acceptance and entire remaining product scope. Continue
from current source and proof; do not mark the whole goal complete.
