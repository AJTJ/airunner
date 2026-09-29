Added The `air-do-less` skill has a "Precedent before change" step: before any change, read the
  solved-issues register and the lifecycle table and write a `## Precedent` section on the bead
  (register entries, states touched, weight, the surgical change). `air-decomposition` says
  the coordinator writes it at filing. This repo sets `lint.sections` in `.beads/config.yaml`
  so `bd create --validate` refuses a task or bug without the section.
