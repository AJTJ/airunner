Added: `air install --write` writes Air's design doc, its register of solved issues and the
  adoption guide to `.air/docs/` from the binary, ships the `air-defect` skill that reads them,
  and `air doctor` names a copy that differs. `air release-check` refuses a release whose
  newest parity sweep predates the last change to `crates/` or `docs/rules/`.
