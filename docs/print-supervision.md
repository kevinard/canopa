# Print worker supervision checks

The Print client gives ordinary workers a 60-second budget and authenticated
workers a 300-second budget. Deadline expiry closes the cancellation pipe and
withdraws authentication. An uncooperative worker gets ten seconds to stop before
forced retirement. Caller cancellation, a dropped future and backend shutdown
also stop the operation; pending retirement keeps its existing ownership guards.

The maintained native fixture exercises the real Rust `Client`, `Broker`, private
credential `Dialog` and supervisor through public APIs. It uses controlled child
messages instead of a scheduler. The child verifies the inherited image's bytes
and seals but does not load CUPS. Credential values use the private channel;
ordinary output and retained receipts contain no credential frames.

`print-supervisor-client.rs` is linked against existing normal/default workspace
artifacts. Its build receipt pins its source, compiler, executable and three
library archives. Reuse that graph; do not compile another dependency selection
or copy the fixture executable into every case directory. The exact standalone
build command and source-only replay are retained in the
[phase68 evidence](../.scratch/xdg-portals/print/supervision-evidence.json).
Fixture debug information is stripped to keep its one executable small.

From the workspace root, with the current build receipt and selected client:

```sh
python3 scripts/test-print-supervisor.py --parent-build BUILD_RECEIPT --library CLIENT_IMAGE --expected-library-sha256 SHA256 --mode controls --artifacts NEW_CONTROL_DIRECTORY
python3 scripts/test-print-supervisor.py --parent-build BUILD_RECEIPT --library CLIENT_IMAGE --expected-library-sha256 SHA256 --mode resources --artifacts NEW_RESOURCE_DIRECTORY
python3 scripts/test-print-supervisor.py --parent-build BUILD_RECEIPT --library CLIENT_IMAGE --expected-library-sha256 SHA256 --mode deadlines --artifacts NEW_DEADLINE_DIRECTORY
```

The fixture needs Linux process/descriptor inspection, executable memfds, private
Unix sockets and Python 3. Deadline mode takes about 310 seconds; it uses the
production budgets without shortening them. Use a new artifact directory for
each invocation. An observation timeout does not prove the job stopped; inspect
or resume the original running process before starting another invocation.

The deadline checks measure both cancellation-pipe EOF and the parent response.
The resource check retains one parent through 42 rounds: warmup, eight repetitions
of successful authentication, pending-prompt cancellation, dropped futures,
invalid replies and helper failure, then an ordinary operation. Each success
uses two private exchanges and one prompt, exercising the request cache. Every
round returns to 11 descriptors, one retained image and no direct children in the
recorded environment. The count is a measured baseline for that process, not a
portable descriptor-count requirement for every desktop.

The current deadline responses are 60.03 and 70.03 seconds for ordinary workers,
and 300.03 and 310.04 seconds for authenticated workers. All 114 unique observed
candidate process identities across controls, resource and deadline receipts are
terminal; private data is removed. The initial sandbox attempt failed at access
to an inherited Unix socket and remains separate failed environment evidence.

This verifies the retained parent's supervision and descriptor ownership. Actual
Iced consent, loaded CUPS behavior, document delivery, device behavior and global
or cross-restart resource limits retain their separate scopes. No application
Rust code, timeout setting or dependency graph changed. Print stays opt-in; see
[remaining work](../.scratch/xdg-portals/print/authentication-connection-next.md).
