# The one gate for ai_runner (air-h4i, owner 2026-08-22). Cheap first so a red shows in
# seconds (backlog #7): fmt, clippy, tests, then `air selftest` on the binary this tree just
# built, never the installed one. No quick/full split until the ledger shows avg verify > 60 s.
# Record it: `air record verify -- make verify`.

.PHONY: verify
verify:
	cargo fmt --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
	cargo run -q -p air -- adopter-check
	cargo run -q -p air -- selftest

# Adopt examples/minimal from scratch with this tree's binary (air-livz): init, install, the
# first green, status and every launcher's --print, then diff what init wrote against the
# example. Needs bd and claude on PATH. Run on demand and by `make release`. Not in `verify`:
# it took 18 to 22 s over three runs on 2026-09-26, most of it bd starting Dolt, and `verify`
# runs before every close.
.PHONY: adoption-check
adoption-check:
	cargo build -q -p air
	sh scripts/adoption-check.sh

# Cut a release (owner, 2026-08-29: "enforce a good release system, so that we draw those lines
# in the sand more readily"). Air had no release concept at all until then: version 0.0.1 since
# the first commit, no tags, and a surface version that could move on nobody's authority.
#
# The LINE is `install::RELEASES`, and since air-mir (owner, 2026-09-06) it is asked in two
# places rather than one. `verify` above asks only that nothing went backwards, because a lane
# appends a surface notice and NO row and that is the normal mid-round state. `air release-check`
# below asks that the notice count and Cargo.toml agree with the last row exactly, and names the
# row to append when they do not. Checking at every verify cost nineteen releases in one day
# and five row collisions between lanes.
#
# The coordinator, at round end:
#   1. append ONE row to install::RELEASES covering every notice since the last:
#      (crate version, surface version, notice count)
#   2. set the same version in Cargo.toml [workspace.package]
#   3. make release (verify, then adoption-check, then the tag)
#
# Steps 1 and 2 in either order: `air release-check` fails until they agree.
.PHONY: release
release:
	@test -z "$$(git status --porcelain)" || { echo "release: tree is dirty; commit first"; exit 1; }
	@test "$$(git rev-parse --abbrev-ref HEAD)" = main || { echo "release: cut releases from main"; exit 1; }
	cargo run -q -p air -- release-check
	$(MAKE) verify
	$(MAKE) adoption-check
	@v=$$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2); \
	git tag -a "v$$v" -m "air v$$v"; \
	echo "tagged v$$v — now: cargo install --path crates/cli"
