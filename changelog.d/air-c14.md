Fixed The machine probe behind `air run` and `air record` read an empty CPU model on arm64 Linux,
  where `/proc/cpuinfo` has no `model name` line; it now falls back to `lscpu`, then to the
  CPU implementer and part, so the run row names the CPU on the pod.
