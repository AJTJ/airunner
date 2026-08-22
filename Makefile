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
