SurfaceChange {
    id: "discharged-clause-names-its-lookup",
    since: "2026-09-06 (air-rud0)",
    headline: "A landing's acceptance report prints `ok (the merge changed <path>) — \
               <clause>` where it printed a bare `ok`. The verdict was already honest and \
               its reason was already on the line beneath; what was wrong is that a \
               reader scanning the verdict column saw a REASON on the branches Air could \
               not read and a bare tick on the branch it could, so the weaker claim wore \
               the stronger form. `ok` means a lookup matched, never that a clause's \
               substance was checked.",
    silent_break: true,
    action: "If anything of yours greps a landing's output for `ok ` followed by the \
             clause text, the lookup now sits between them, and the discharged clause is \
             one line rather than two. `MISS` and `?` are unchanged, reason on the line \
             beneath. `--json` is unchanged entirely: `Verdict::Discharged` has carried \
             `how` since it was written, which is why this cost no new fact.",
}
