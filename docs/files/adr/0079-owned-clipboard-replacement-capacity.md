# 0079: Bounded capacity for owned clipboard replacement

Status: scoped implementation verified, 2026-10-19. Extends ADR0075/0077. The full
product brief and resident-host work remain active.

Native13697 reproduced a clipboard that could not be replaced: with the Files
child limited to128 descriptors, Copy of32 selected files published successfully,
but Copy of one different file was refused by the ordinary source-pin budget.
Independent wl-paste still returned all32 original URIs. The narrow Browser
showed the capacity error. The old intent was safe but held its entire allowance
indefinitely, including between Browser Windows.

Use `OperationJobs::replace_clipboard` for a deliberate replacement of a still
owned intent. The core accepts the old opaque PreparedSelection, validates its
originating runtime, and retains it throughout preparation. The frontend supplies
that handle only while its native ownership token matches; it implements no
descriptor arithmetic. Publication and obsolete-result handling retain ADR0077's
transport tokens and invocation serials. A failed preparation/publication does
not discard the previous native intent or release its original observations early.

Ordinary captures retain their process-wide limit of4096 pins and one quarter
of the current descriptor soft limit. New selections retain the same individual
limit. Replacement gets temporary process credit no greater than the previous
selection's actual shared reservation, capped by the ordinary allowance. Thus
all source pins together stay below8192 and half the descriptor allowance even
with overlapping/repeated replacements or cloned old handles. Independent queued
work and external selections receive no replacement credit. Other pending work
can legitimately require replacement to wait until some reservations settle.

The previous selection's process identity is an opaque core-owned token, with
clones sharing the original token and observations. A foreign/stopped runtime's
selection cannot supply credit to a new runtime. This is resource ownership,
not filesystem mutation, source retention, replay or original-media authority.
Failed partial capture releases all its pins and descriptors. EMFILE/ENFILE still
refuse the whole preparation. Old queued Pastes retain their own original guards
and remain safe when native ownership advances to a newer selection.

Public tests use actual private files and an isolated128-descriptor child through
the existing prepared-clipboard/operation seams (ADR0001). Verify full-to-full
replacement, old-handle identity protection during overlap, refusal/cleanup,
ordinary-budget restoration, foreign-runtime refusal and real Paste/Undo. Native
acceptance uses the same limit only on the Files child, observes the actual
clipboard independently and verifies marker bytes and Undo in both themes.
These are acceptance targets; actual results are recorded separately.

Production/fixture source04 and binary1ea23cab pass the public regression94083
(one parent plus its isolated128-descriptor helper), full no-desktop13990
(455 parent tests plus2 isolated helpers),50 Iced tests78543, default
all-target Clippy23992, formatting and normal build70455. Clippy retains three
existing unused-helper warning groups. Both scoped review axes report0 findings
after verifying all9 changed-file hashes.

Native65632 passes26 checks across six cases: new capacity dark100/light125
(6 checks) and existing stale-source Copy/Cut in both themes (20). All six
application logs are empty. Independent clipboard reads observe the exact marker
URI replacing the old32-file payload; literal byte checks and shared Undo preserve
all originals. Scoped visual inspection confirms the error banner is gone after
replacement and the light125 destination is empty after Undo. The genuine native
red13697 and its original payload/error capture remain preserved. Source05 changes
documentation only;550 input hashes and the source archive/binary gzip are verified.
No resident startup, activation or whole-product completion is established here.
