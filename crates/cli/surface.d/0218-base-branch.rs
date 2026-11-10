SurfaceChange {
    id: "base-branch",
    since: "2026-11-03 (air-h7l)",
    headline: "`\"base_branch\": \"<name>\"` in `.claude/air.json` (new, default `main`): the \
               branch the fleet lands onto. Every gate, cut, landing and notice reads it where \
               it said main, and names it: `<base> moved to`, `contains <base>`, `git merge \
               <base>`. `air status` prints it when set, and `--json` carries `base_branch`.",
    silent_break: false,
    action: "Nothing for a repo landing onto main. To land onto another branch, set the key and \
             check that branch out in the main checkout; `.air/roles.md` reads main as the \
             base. Landing the base onto the real main stays the owner's merge.",
}
