Fixed `air run --pull` through `kubectl exec` reported "the tar stream failed" and recorded nothing
  pulled although the files arrived: the local tar closes the pipe at the end-of-archive marker
  and the prefix dies of SIGPIPE. The unpack is now the verdict; a sender exit code after a good
  unpack is reported as the target's tar failing.
