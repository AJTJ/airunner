Fixed `air status` showed a session's model from the first transcript line carrying the key,
  whatever the record kind, so the coordinator read as Opus while it ran Fable. The model is
  now the newest assistant record's, and a `/model` switch after launch shows on the next hook.
