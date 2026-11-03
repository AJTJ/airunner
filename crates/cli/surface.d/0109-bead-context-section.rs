SurfaceChange {
    id: "bead-context-section",
    since: "2026-09-25 (owner, from an adopter)",
    headline: "The installed `air-decomposition` skill asks every bead to end with a \
               `## Context` section naming the skills the worker invokes first, the files \
               and doc sections to read first, and any doc the work must update; \
               `.air/roles.md` tells the worker who claims it to do so.",
    silent_break: false,
    action: "A repo that added its own context section to its copy of the skill loses that \
             edit on `air install --write`; keep project-specific skill lists in the repo's \
             own intake guide and name them in the section.",
}
