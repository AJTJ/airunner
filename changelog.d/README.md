# Changelog fragments

One file per bead, `changelog.d/<bead-id>.md`, instead of a line in `CHANGELOG.md`. Every bead
editing the same heading made every landing conflict with every other branch (air-be6d: nine
lane drops on 2026-09-28). A fragment is a file no other bead touches.

The first word is the heading, `Added`, `Changed`, `Removed` or `Fixed` (a colon after it is
fine), and the rest is the entry in plain lines, wrapped at 96 columns with two-space
continuation lines:

    Fixed `air close` named the wrong bead when two ids shared a prefix; it names the one
      bd resolved.

`make changelog` folds every fragment into `CHANGELOG.md` under `[Unreleased]` and deletes
them; `make release` refuses while any is left. `sh scripts/changelog-assemble.sh --dry-run`
shows the result and changes nothing.
