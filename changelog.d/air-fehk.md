Changed The `main moved` notice no longer tells a worker `git merge main` brings the landing into
  its branch. Workers read it as an instruction and merged mid-batch, losing a batch cycle; the
  notice states what landed and which files changed, and the worker decides.
