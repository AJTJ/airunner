Fixed `make changelog` refused every fragment after a release, because the release leaves
  `[Unreleased]` with no headings; a missing heading is now added under `[Unreleased]` in the
  standard order.
