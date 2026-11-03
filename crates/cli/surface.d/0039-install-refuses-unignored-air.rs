SurfaceChange {
    id: "install-refuses-unignored-air",
    since: "2026-09-05 (air-6di)",
    headline: "`air install --write` and `air init --write` REFUSE while `git check-ignore \
               -q .air` fails, naming the fix. It used to be advice.",
    silent_break: true,
    action: "A repo that never ignored `.air/` now gets exit 2 from `install --write`: \
             `echo '.air/' >> .gitignore` and re-run. The reason is the ledger: since \
             `messages-table` it holds the text of every agent-to-agent message, and one \
             `git add -A` would commit it. There is no bypass flag.",
}
