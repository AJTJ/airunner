# The one gate for ai_runner (air-h4i, owner 2026-08-22). Cheap first so a red shows in
# seconds (backlog #7): fmt, clippy, tests, then `air selftest` on the binary this tree just
# built, never the installed one. No quick/full split until the ledger shows avg verify > 60 s.
# Record it: `air record verify -- make verify`.

.PHONY: verify
verify:
	cargo fmt --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
	cargo run -q -p air -- selftest

# Cut a release (owner, 2026-08-29: "enforce a good release system, so that we draw those lines
# in the sand more readily"). Air had no release concept at all until then: version 0.0.1 since
# the first commit, no tags, and a surface version that could move on nobody's authority.
#
# The LINE is `install::RELEASES` and the test that reads it, and that test runs in `verify`
# above — so the rule holds on every run, not only when someone remembers this target. What is
# left here is the mechanical part: refuse a dirty tree, verify, tag what was verified.
#
#   1. append a row to install::RELEASES: (crate version, surface version, notice count)
#   2. set the same version in Cargo.toml [workspace.package]
#   3. make release
#
# Steps 1 and 2 in either order: `make verify` fails until they agree.
.PHONY: release
release:
	@test -z "$$(git status --porcelain)" || { echo "release: tree is dirty; commit first"; exit 1; }
	@test "$$(git rev-parse --abbrev-ref HEAD)" = main || { echo "release: cut releases from main"; exit 1; }
	cargo run -q -p air -- release-check
	$(MAKE) verify
	@v=$$(grep -m1 '^version = ' Cargo.toml | cut -d'"' -f2); \
	git tag -a "v$$v" -m "air v$$v"; \
	echo "tagged v$$v — now: cargo install --path crates/cli"
