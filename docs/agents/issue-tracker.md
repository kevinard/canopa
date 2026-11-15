# Issue tracker

Issues, feature requests and pending work live in GitHub Issues for
`kevinard/canopa`: <https://github.com/kevinard/canopa/issues>. Use `gh`:

```sh
gh issue list --repo kevinard/canopa --label "area: files"
gh issue view 42 --repo kevinard/canopa
gh issue create --repo kevinard/canopa --title "Files: …" --body-file BODY.md --label bug --label "area: files"
```

Write each issue so it stands on its own: context, the problem, the current state
in code (file paths), a proposed approach and acceptance criteria as a checklist.
Prefix the title with its area (`Shell:`, `Notify:`, `Files:`, `Portals:`,
`Print:`, `Settings:`, `Session:`, `Launcher:`, `CI:`, `Packaging:`…) and apply
the labels from [triage labels](triage-labels.md). Comment on the issue as work
progresses; close it from the pull request that resolves it.

Do not keep plans, tickets or run logs in the repository. Specs that need review
belong in the issue or the pull request description. Durable decisions go into
[decision records](../adr/README.md) and the architecture pages.
