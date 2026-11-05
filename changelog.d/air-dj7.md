Fixed the drain test `a_real_hang_is_killed_and_reported` failed on Linux: its `sh -c "sleep 5"`
  forks `sleep` there, the kill reached `sh` alone, and the timeout path waited the whole
  stall for the pipes. The fixture now execs `sleep`, and the git.rs timeout path records the
  grandchild stall as a known gap with the condition for bounding the drain.
