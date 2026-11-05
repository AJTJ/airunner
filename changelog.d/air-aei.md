Added `.claude/air.local.json`, gitignored by the adopter and read beside `.claude/air.json`
  from the main checkout: its top-level keys replace the committed file's, so a machine's
  `targets` and allow lists stay out of git history and take effect when written, not when a
  landing carries them (twice on 2026-11-05 a `targets` landing verified on the old target).
  `air status` names an unknown key the overlay set with its file, reports an overlay that
  does not parse, and warns once if git tracks it.
