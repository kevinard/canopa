# Print quoted credential/challenge interoperability — sealed

Final state: all validation processes are terminal. The optional client fix is
scoped-verified; the full portal goal remains active and Print remains opt-in.
See ADR0103, print/quoted-authentication-evidence.json and
print/authentication-connection-next.md. Historical launch notes below retain
their original observations; their LIVE markers are superseded by this state.

The final Send-only quoted-account wrong-once TLS check passes, as do all four
workspace post-stages (Settings, CLI, configuration, vendor). Native candidate
receipts record 343 unique terminal PIDs with private data removed. Portal437/55
pass in the required workspace run; that run remains non-green with the existing
ten Files integration failures and one Files video failure. The video case passes
an isolated compiled retry; no cause or fix is claimed. A host process inventory
confirms no validation build, scheduler, sandbox or worker remains running.

The current source guard is target/portal-print-66/source-guard-01.json, 1310
operational inputs, explicitly recording the non-green workspace outcome. The
Rust binary remains EF722d77...; client49cedda4... is the new canonical image.
Source-only replay and exact pins are retained, no per-fixture ELF copies or cache
purge. Net storage growth is about15MB, with about6.18GB free. Next: existing outer
supervision/resource, runtime packaging/NOTICE and driverless/device/Flatpak gates.
Automatic Qt routing remains deferred as requested. No pending user question,
approval, build or native process.

Previous goal turn PROGRESS: phase65 private-reply lifetime fix sealed. All old
handles terminal. Revalidated1306 source inputs/EF722 binary at turn start; CUPS
imageE0 unchanged. No product Rust/vendor edit/build in this phase yet.

New strict independent byte oracle /tmp/canopa-print-66/digest_oracle.py uses
RFC9110 section5.6.4 and RFC7616 quoted fields/unq. Three published MD5/SHA256
reference vectors, two golden quoting checks and seven malformed cases pass.
No urllib parameter parser; no auth headers/responses/secrets retained in receipts.

Actualworker plaincontrols Basic/MD5/SHA256/legacy pass with sealedE0 mapping;
control01/session84037 terminal0.48-case positive interoperability baseline now
LIVE (record returned exec handle). Source-only fixture quoted_worker.py; private
HTTP+emptyIPP protocol endpoint only, no CUPS scheduler/Iced/PAM/document/device
acceptance. It measures authorization AND exact realm exposed to private prompt.

Source inspection finds raw string formatting/quoted parameter scanners worth
checking, but do not claim defects before baseline measurements. RFC sources:
https://www.rfc-editor.org/rfc/rfc7616#section-3.4.5
https://www.rfc-editor.org/rfc/rfc9110#section-5.6.4

Budget baseline target66/baseline.json, reserve5GB/cap900MB. Existing rootgraph/
checksum stores; no cachepurge/ELFcopies. User Print-first/Qtdeferred; full18 active.

Baseline48 measured18pass/30fail, session21409 terminal1; selectedE0 mapping passes
for allworkers, all52 control+matrix observed workers terminal/privatecleanup.
Basic realm quote/backslash exposes incorrect private realm despite HTTPsuccess;
Digest username/realm/nonce/opaque quote cases fail independently. Investigation01
records3 ranked hypotheses. Verified all1301 stable CUPS sources against oldrecipe
and official archive; canonical/provenance/build/source-input/7source snapshots
in target66/baseline-overlay. Full vendor-maintenance instructions read before edits.
Next minimal quoted-field parse/format fix in canonical CUPS overlay, no Rust edit.

Candidate canonical7file patch fixes quoted decoding in auth.c/http.c and complete
Digest escaping/sizecheck in http-support.c. Same-cache transition verified1301:
recipebacfe547.../patch0cd02535...; library49cedda4...841160bytes builds/pinned once,
source49 image onlyworker, no Rust edit/rebuild. build87368 terminal0. Existing
CPPwarningsfortifyO0/const at untouchedhttpAssembleURI retained.
Same48candidate cases PASS/session37821 terminal0; currentRustEF unchanged, only
exactprovenance guarddifference permitted and recorded in candidate-source-continuity01.
New maintained scripts/test-cups-authentication.py plus strictoracle under
scripts/desktop-fixture; selftest addedtoworkspace gate.77case candidate and23case
oldE0subset now LIVE (record returned handles). Selftest3vectors/2golden/7malformed
PASS. Need TLS, actualIced bothappearances, requiredworkspace gate afterfinaledits,
canonical/license/NOTICE/tooling checks. Fullgoalactive Printoptin Qtdeferred.

Maintained tool: candidate77 PASS/session15102 terminal0; E0subset23 retains20
failures/session38421 terminal1. TLS77 PASS/session82504 terminal0. New source tool
selftest additionally verifies Basic scheme case and rejects empty native selection.
Old quoted-fake-Basic refusal cases generated Digest, NOT Basic: do not claim a
measured Basic downgrade. New49 sends no Authorization for unsupported qop there.
Checkscripts15 and offlinevendor PASS/session82087 terminal0. Full official archive
materialization7files/LICENSE/NOTICE/all1301 matches compiled source; transient
source-only tree removed, session28760 terminal0.
Required workspace gate now LIVE (record returnedhandle); source/tooling frozen
under workspace-gate01 inputs. Need inspect actual exit/failures; preserve exact
featuregraph/noFailFast and continue skipped poststages only if cargo fails.
No newRust compile/profile/source edit; EF stillcurrent. RealIced49 quoted flows
bothappearances remain, then docs/evidence. Fullgoalactive Printoptin Qtdeferred.

CURRENT onlylivehandle: required workspace gate session91868. All native/control/
TLS/archive/build/quick-check handles consumedterminal. Rootoperational sources
frozen in workspace-gate01 guard; do not edit them until gate finishes. NoIced
49regression hasstarted yet; stage source-only fixture copies first.

Workspace91868 stillLIVE: Fileslib371pass/1failure/34ignored, video paused-frame
case unexpectedPaused0ns atplayback/tests.rs24. Preservefailure and retryexact
compiledcase AFTER broad run, do not assume sameoldten-only. Filesintegration
continues under --noFailFast. No Files source edit.
Iced quoted49 SHA256 dark now LIVE (record returned handle); source-only /tmp66/iced.py
uses actualpublicfrontend/ownedCUPS, quotedUTF8user/realm/nonce/opaque, three consent
transitions and prepared4page file, no device/physical claim. Root code frozen.

Workspace91868 TERMINAL1/gate exit101; allnoFailFastexecutablescompleted. Correct
workspace-failures02 separates knownfiltered crashhelper from top-level lists:
existingtenFiles integration failures plusonevideo libraryfailure. Rawnaive audit01
retained. Exactcompiled video retry89078 PASS1.20s fromFilescwd; no sourcefix/cause
claim. CIremainsnon-green. Poststages now LIVE (record returnedhandle).
ActualIced quotedSHA256 dark65408/light68675 PASS3groups each, selected49image
with quotedUTF8user/realm/nonce/opaque, prepared4pagefile/nojobs. Allnativecleanup.
Finaltool77session31052 PASS. Protectedquoted-account Create/PAM control+Rust
Digest session91619 PASS (principal01); initialfixturealias-template prepfailed
beforeanyservice, logprotected-create01 retained; correctedsource prep thenran02.
Quotedaccount send-only wrongonceTLS now LIVE (record returnedhandle).
No operational source/Rust/library change aftergateguard; only /tmp fixtures/docs.
