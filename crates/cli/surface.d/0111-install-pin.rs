SurfaceChange {
    id: "install-pin",
    since: "2026-09-25 (owner, air-4usc)",
    headline: "`air install --write --pin` copies the running binary to `.air/bin/air` and \
               points the hooks and the `.mcp.json` channel at that copy by absolute path. \
               The launchers then put `.air/bin` first on PATH in every session they start, \
               and `air status` and `air doctor` name the pin and warn when the `air` on \
               PATH has a different surface version. `--unpin` goes back to PATH. A pinned \
               repo refuses a plain `air install --write` until `--pin` or `--unpin` is \
               given.",
    silent_break: false,
    action: "",
}
