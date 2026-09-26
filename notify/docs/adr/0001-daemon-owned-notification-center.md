# Daemon-owned notification center

Notify owns notification receipt, policy, history, and notification center state. Niribar hosts the notification center through its external panel provider integration rather than implementing notification policy or history inside the bar, keeping Notify usable without Niribar and keeping notification behavior out of Niribar's domain. The first notification center targets Niribar's current external panel provider protocol before requiring richer panel primitives, and does not require a dedicated Niribar notification module.
