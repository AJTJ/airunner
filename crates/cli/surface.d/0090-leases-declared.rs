SurfaceChange {
    id: "leases-declared",
    since: "2026-09-25 (from an adopter's fleet protocol)",
    headline: "`.claude/air.json` can declare which commands need which lease, \
               `\"leases\": {\"runtime\": [\"make api*\", \"adb *\"]}` in the deny-rule \
               pattern syntax. Air's PreToolUse hook then refuses such a command from a \
               worker that does not hold the lease (advises the coordinator; never the \
               owner), and `air lease needs \"<cmd>\"` says what a command needs.",
    silent_break: false,
    action: "Optional; nothing changes without the key. A repo with its own lease guard can \
             move its patterns into `leases` and retire the guard, keeping one store and one \
             checker. Run both only while comparing them with `air lease needs`.",
}
