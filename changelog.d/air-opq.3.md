Added every `air run` is recorded in the ledger's `runs` table with the machine it ran on
  (hostname, `AIR_IMAGE` if the target sets one, arch, nproc, CPU model), beside the session,
  commit, tree, command, exit, times, output size and target; `air run --json` prints the row.
