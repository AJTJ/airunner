SurfaceChange {
    id: "model-from-assistant-records",
    since: "2026-11-09 (air-ius)",
    headline: "The model `air status` prints for a session is read from the newest assistant \
               record in its transcript. It was the first line of any kind carrying the key, so \
               a request-shape record that mentioned a model id in a tool schema showed the \
               coordinator as Opus while it ran Fable, and a `/model` switch after launch never \
               showed.",
    silent_break: false,
    action: "None. The next hook of each session rewrites the recorded model.",
}
