SurfaceChange {
    id: "roles-base-branch",
    since: "2026-11-06 (air-6el)",
    headline: "`.air/roles.md` writes `git merge <base branch>` where it wrote `git merge main`, \
               and its Every-role section says the name is `main` unless `.claude/air.json` sets \
               `base_branch`, read from `air status` (`base branch: <name>`). On a repo with the \
               key set, two workers copied `git merge main` from the old text and it failed with \
               no hint.",
    silent_break: false,
    action: "Nothing; `air install` rewrites `.air/roles.md`. A repo that sets `base_branch` \
             gets the text that matches it.",
}
