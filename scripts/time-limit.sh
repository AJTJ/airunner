#!/bin/sh
# Run a command, and kill it with everything it started after $1 seconds (exit 124).
# macOS has no `timeout`; a plain alarm would kill cargo and leave its test binaries running.
secs=$1; shift
exec perl -e '
  my $secs = shift; my $pid = fork;
  if (!$pid) { setpgrp(0, 0); exec @ARGV or die "exec: $!" }
  $SIG{ALRM} = sub { kill "KILL", -$pid; print STDERR "killed after $secs s: @ARGV\n"; exit 124 };
  alarm $secs; waitpid($pid, 0); exit($? >> 8);
' "$secs" "$@"
