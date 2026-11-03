SurfaceChange {
    id: "lease-free-told",
    since: "2026-09-26 (air-1vri.3)",
    headline: "When a lease is released (`air lease release`) or broken (`air lease \
               break`), each worker that was refused it hears \"<lease> is free\" once, \
               oldest want first (event line `fanout / lease-free`). Its want is cleared \
               when the message is delivered or when it takes the lease.",
    silent_break: false,
    action: "",
}
