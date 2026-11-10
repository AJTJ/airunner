SurfaceChange {
    id: "messages-table",
    since: "2026-09-05 (air-srv)",
    headline: "`.air/ledger.db` now holds the CONTENT of every `SendMessage` a session \
               sends, in a `messages` table (schema v13). The event line is unchanged: \
               recipient and byte count, never the text.",
    silent_break: false,
    action: "Nothing to run; the table is created on the next hook. Know that the ledger \
             file is now a transcript of agent-to-agent traffic: `sqlite3 .air/ledger.db \
             'select * from messages'` reads it, `air doctor` counts it. Only the send side \
             is a tool call, so a message from another fleet is in that fleet's ledger.",
}
