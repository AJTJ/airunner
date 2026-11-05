Fixed `air selftest`'s base_branch probe landed without a git identity and failed with exit 128
  on a host with no global git config: every air the selftest spawns now carries the same
  identity its git fixtures set, from one constant.
