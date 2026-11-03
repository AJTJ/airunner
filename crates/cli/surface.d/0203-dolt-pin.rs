SurfaceChange {
    id: "dolt-pin",
    since: "2026-10-22 (owner ruling)",
    headline: "Dolt is pinned at 2.3.5 for projects whose bd runs in server mode; `air \
               doctor` reports a mismatch and refuses nothing.",
    silent_break: false,
    action: "In a server-mode project, install Dolt 2.3.5 from its release: `curl -L \
             https://github.com/dolthub/dolt/releases/download/v2.3.5/install.sh | sudo bash`. \
             Homebrew only has the latest Dolt and bd.",
}
