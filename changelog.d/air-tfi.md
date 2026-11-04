Added `"targets"` entries take an `exec` list, the argv prefix that runs a command on the
  machine (such as `kubectl exec -i <pod> -c <c> -n <ns> --`); `air run` and `air record` run
  every step through it and push through it as git's `ext::` remote. `"kind": "ssh"` stays as
  the short form for `ssh user@host`.
