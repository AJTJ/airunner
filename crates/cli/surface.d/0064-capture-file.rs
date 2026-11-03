SurfaceChange {
    id: "capture-file",
    since: "2026-09-06 (air-45pw)",
    headline: "`air capture --file <path>` files a whole file as the capture text, \
               untruncated. The positional stays and most captures are still one-liners; \
               exactly one of the two, and passing both or neither is refused naming both.",
    silent_break: false,
    action: "Nothing to re-run and nothing refuses that did not before. Worth knowing if \
             your workers capture through the CLI rather than the `air_capture` MCP tool: \
             a finding long enough to be worth writing goes through the harness's command \
             classifier as a command line, and one was refused for its shape — the worker \
             shortened the finding in order to file it. A shortened capture looks exactly \
             like a capture, so the loss is silent. Point your rules at `--file` for \
             anything longer than a line. A fleet capturing through the MCP tool never \
             sees this, which is why it took an adopter to find it.",
}
