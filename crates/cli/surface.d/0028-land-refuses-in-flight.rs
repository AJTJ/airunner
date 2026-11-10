SurfaceChange {
    id: "land-refuses-in-flight",
    since: "2026-09-05 (air-1bm)",
    headline: "`air land` REFUSES while any verify is in flight, naming each run and its \
               pid. It used to warn and land anyway. `--despite-inflight` lands regardless \
               and is recorded on the landings row (`despite_inflight`, schema v16) and the \
               event line.",
    silent_break: false,
    action: "A landing that used to print a warning and proceed now exits 2 with the runs \
             named. Wait (`air status` shows when they exit), stop one by pid (`kill \
             <pid>`, never `pkill -f`), or pass `--despite-inflight` knowing it destroys \
             those runs. The adopter lost 1,199 s of finished verify to the warning on \
             2026-08-30 and an operational rule did not hold; the override count is what \
             decides whether the refusal stays.",
}
