#!/usr/bin/env python3
"""Mark the trial's scratch copy as trusted in Claude Code's `~/.claude.json` (air-zau), so no
session of the trial fleet stops on the folder-trust dialog, on any machine, with no step for
the owner.

Usage: trust-copy.py DIR

Writes `projects[<DIR>].hasTrustDialogAccepted = true`, under the path's physical form, which
is how Claude Code keys the entry (`/private/var/...` on a Mac where `$TMPDIR` says `/var/...`).
The file is `$CLAUDE_CONFIG_DIR/.claude.json` when that is set, else `$HOME/.claude.json`, the
same rule the launcher reads by. Nothing else in the file changes: every other key is read and
written back as it was, the entry's own other fields stay, the file keeps its mode, and the
write goes through a temp file in the same directory and a rename, so Claude Code's own writes
never see a half-written file. A file that is not JSON is left alone and the exit is 1. An
absent file is created with the one entry.

Only the trial script runs this, for a directory it has just built from Air's own
examples/minimal; the launcher still refuses an untrusted repository and never writes the key
(docs/research/folder-trust.md, option D narrowed; owner ruling 2026-11-07). The trial stopped
on the dialog three times on 2026-11-05 and 2026-11-06, hours each (air-fzl and after).
Removed when Claude Code offers a session-level flag that pre-trusts a directory.
"""
import json
import os
import stat
import sys
import tempfile


def config_path():
    base = os.environ.get("CLAUDE_CONFIG_DIR") or os.environ.get("HOME", "")
    return os.path.join(base, ".claude.json")


def main(argv):
    if len(argv) != 2:
        print("usage: trust-copy.py DIR", file=sys.stderr)
        return 2
    path = os.path.realpath(argv[1])
    file = config_path()
    mode = 0o600
    data = {}
    if os.path.exists(file):
        mode = stat.S_IMODE(os.stat(file).st_mode)
        with open(file, encoding="utf-8") as f:
            text = f.read()
        try:
            data = json.loads(text)
        except ValueError as e:
            print(f"trial: {file} is not JSON ({e}); left as it is", file=sys.stderr)
            return 1
        if not isinstance(data, dict):
            print(f"trial: {file} is not a JSON object; left as it is", file=sys.stderr)
            return 1
    projects = data.setdefault("projects", {})
    if not isinstance(projects, dict):
        print(f'trial: {file}: "projects" is not an object; left as it is', file=sys.stderr)
        return 1
    entry = projects.setdefault(path, {})
    if not isinstance(entry, dict):
        print(f"trial: {file}: the entry for {path} is not an object; left as it is", file=sys.stderr)
        return 1
    if entry.get("hasTrustDialogAccepted") is True:
        print(f"trial: {path} is already trusted in {file}")
        return 0
    entry["hasTrustDialogAccepted"] = True
    directory = os.path.dirname(file) or "."
    fd, tmp = tempfile.mkstemp(prefix=".claude.json.", suffix=".air-trial", dir=directory)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(data, f, indent=2)
            f.write("\n")
        os.chmod(tmp, mode)
        os.replace(tmp, file)
    except BaseException:
        try:
            os.unlink(tmp)
        except OSError:
            pass
        raise
    print(f"trial: trusted {path} in {file} (one key, hasTrustDialogAccepted)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
