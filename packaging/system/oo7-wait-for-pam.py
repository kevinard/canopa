#!/usr/bin/python3 -I
"""Wait for oo7's PAM listener as part of its bounded user-service startup."""

import os
import sys
import time


def main(runtime):
    configured = os.environ.get("OO7_PAM_SOCKET")
    paths = {configured} if configured else {f"{runtime}/oo7-pam.sock", f"{runtime}/oo7/pam.sock"}
    while True:
        # A connect-and-close probe would be logged as an invalid PAM request.
        # systemd shares the daemon's network namespace with ExecStartPost.
        with open("/proc/net/unix", encoding="utf-8", errors="surrogateescape") as sockets:
            next(sockets)
            for line in sockets:
                fields = line.rstrip("\n").split(maxsplit=7)
                # Flags 0x10000 and type 0x0001 identify a listening stream.
                if len(fields) == 8 and fields[3:5] == ["00010000", "0001"] and fields[7] in paths:
                    return
        time.sleep(0.05)


if __name__ == "__main__":
    main(sys.argv[1])
