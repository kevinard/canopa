# Validation storage control

The repeated disk exhaustion was worsened by dependency-feature changes during
continuations, random Cargo source identities in fork tests, per-run compositor
copies, and obsolete full-size Rust test executables. The user explicitly asked
to stop accumulating artifacts after freeing space.

Completed cleanup retains every test log, receipt, source patch and capture.
Identical finished compositors now resolve to one read-only binary per checksum.
16 historical portal executables are retained as verified lossless `.gz`
archives beside their original paths. The exact paths, hashes, permissions and
allocated-byte savings are in `historical-archives.json`; `deduplication.json`
records all retained compositor paths. Four previous-stage binaries in `/tmp` were removed after verifying their
lossless archives. Two archives already existed; two intermediate binaries were
compressed once before removal.
Current pointer-matrix executables remain available for the next native checks.
Root `target/debug` and active executable inodes were preserved.

To replay an archived executable, decompress its listed archive to a temporary
file, verify the SHA-256 against the manifest, restore its recorded executable
mode, then rename it to the original path. Remove that restored copy when the
replay is finished. Do not preserve another full raw copy per run.

Persistent fixes:

- Both desktop and media fixtures use the checksum-addressed compositor store.
  Source changes or corrupt cached bytes are rejected before a run starts.
- Fork and pipe tests synchronize prepared sources at stable paths under
  `target/vendor-test-sources`; unchanged transformed manifests keep mtimes.
  Each fixture holds a lock while preparing and compiling its sources.
- Workspace Rust tests use `--no-fail-fast`. Interrupted validation retains its
  original feature selection instead of excluding an application and compiling
  another unified dependency graph.
- The current continuation has a six-billion-byte growth budget and an
  eight-billion-byte free-space floor. It stops its own child process group at
  either boundary instead of filling the disk or requesting more cleanup.

Validation: 12 desktop/media artifact checks, 15 check-selection checks, two
source-synchronization checks, Python compilation and shell syntax checks pass.
No Rust application source changed in this storage correction. Existing Files
failures and other incomplete acceptance gates remain recorded separately.

The build guard stopped at 6,005,809,152 bytes of net growth, leaving about 23 GB
free. No more compilation ran. The 17 completed test executables then pass from
their correct crate working directories: 2035 tests pass and nine are ignored.
An initial direct runner used workspace cwd, causing three Notify packaging
ENOENT failures; the corrected full ready set passes with the same binaries.
`validation.json` records this correction and the remaining gate work. The full
workspace gate remains incomplete, and the original ten Files failures remain.
