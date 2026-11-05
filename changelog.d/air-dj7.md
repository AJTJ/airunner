Fixed a git call that timed out waited for whatever still held its pipes before reporting
  the timeout: on Linux a killed `sh` leaves its `sleep` grandchild holding them, so the drain
  test took the whole stall and every verify on the dev pod was red. The timeout path now
  returns at once, git's children run as process-group leaders so its helpers die with it, and
  the bd crate's twin already behaved this way (air-19u).
