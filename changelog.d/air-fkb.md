Changed `make trial` starts the trial fleet itself and needs no keystroke after it: the request
  text is the coordinator's launch prompt (`air coordinator -- "<text>"`, a positional that now
  leads argv instead of landing among the deny values), the development-channels prompt in each
  pane is answered by the script, and `TRIAL_DIR` defaults to one fixed path the owner trusts
  once.
