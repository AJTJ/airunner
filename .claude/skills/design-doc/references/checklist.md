# Checklist

Run after writing or updating the design doc. Each line is a yes or a fix.

Truth
- The header names the commit and the binary version, and both match the tree you read.
- Every number was produced by a command run today, and the command is shown or cited.
- Every behaviour claim points at a file, a command, or a table.
- Nothing outside the TODO section describes a planned or proposed state; proposals are TODO
  lines, not files.

Diagrams
- Each structure and each flow described in prose has a diagram, and the diagram uses the
  glossary's names.
- No diagram has more than about twelve nodes or needs a legend.
- Refusals and recorded facts appear on the sequence diagrams.

Coverage
- Every CLI command, server tool, hook event, table, environment variable, and file path the
  system exposes appears in the interfaces or data sections.
- Every role that runs the system has its permissions stated.
- The failure model says, for each dependency, what happens when it is absent.

Prose
- It reads like ordinary technical writing: complete sentences, no fragments, no aphorisms.
- One name per thing, used everywhere.
- History, argument, and evidence have been moved out, with links, rather than compressed.

Index
- `docs/README.md` and `CLAUDE.md` point at the doc.
