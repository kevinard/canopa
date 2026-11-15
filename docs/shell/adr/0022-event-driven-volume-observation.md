# Event-driven volume observation

The bar's default-output volume observer waits for Pulse context-state changes
and receiver cancellation. Both wake its connection-owning thread. Volume
notifications continue to arrive through Pulse subscription callbacks. A parked
thread preserves a wake that arrives between the state checks and the wait; there
is no periodic idle connection check. Initial connection establishment retains
its existing bounded deadline. Audio panel device observation remains separately
owned under ADR 0016.

This avoids idle audio polling while retaining volume recovery and prompt
cancellation. The performance report (docs/performance.md in the git history at commit
cb300936) has measurements and native checks.
