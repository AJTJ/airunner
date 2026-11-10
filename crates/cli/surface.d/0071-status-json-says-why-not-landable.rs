SurfaceChange {
    id: "status-json-says-why-not-landable",
    since: "2026-09-06 (air-72t7)",
    headline: "`air status --json` carries `land_skipped` and `land_errors` beside \
               `landable`, all three from ONE selection so they cannot disagree. It used \
               to say which branches can land and never why the others cannot, and a real \
               failure inside selection — git or the ledger — reached no caller at all and \
               read as an empty queue.",
    silent_break: false,
    action: "Nothing to run and nothing existing changed; the human rendering is \
             untouched. A script that inferred \"not in `landable` means not landable\" \
             can now read the check, the detail and the fix, and should treat a non-empty \
             `land_errors` as \"could not tell\" rather than as an empty queue.",
}
