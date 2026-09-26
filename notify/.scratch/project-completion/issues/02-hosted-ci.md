# Establish hosted workflow evidence

Status: ready-for-human

The user confirmed no GitHub repository exists. Select/create the repository and
choose a dedicated Linux runner machine before registration or dispatch.

Follow `docs/hosted-ci.md`, provision the compatible Niribar checkout, and register
the `canopa-desktop` runner. Run both configured workflows and record passing run
URLs, commit IDs and retained graphical artifacts. Local test/desktop passes and
runner preflight do not close this gate.
