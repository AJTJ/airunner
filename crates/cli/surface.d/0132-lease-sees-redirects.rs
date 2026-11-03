SurfaceChange {
    id: "lease-sees-redirects",
    since: "2026-09-26 (0.4.6 live trial, D4)",
    headline: "The lease check removes redirections (`2>&1`, `>&2`, `> file`, `>> file`, `2> file`, \
               `&> file`, `< file`) before matching, so `sh serve.sh 2>&1` needs the same \
               lease as `sh serve.sh`.",
    silent_break: true,
    action: "",
}
