SurfaceChange {
    id: "red-member-held",
    since: "2026-09-28 (air-hyli)",
    headline: "A branch whose head was the only member of a red batch is not batch-ready \
               until its worker commits past that head, read from the batch's recorded \
               members. Before, only a red recorded at the head itself counted, which a \
               batch built on main never is, so the lane merged the same red again.",
    silent_break: false,
    action: "",
}
