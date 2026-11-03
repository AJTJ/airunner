Fixed `air batch cut --only` and `--without` could not split a red batch: its members were
  already held out of the set the split chose from. A split now chooses among the batch-ready
  branches and the newest red batch's members at their red-held heads.
