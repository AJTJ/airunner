Changed `.air/roles.md` tells the coordinator how to reach a worker about a bead it holds
  now: an addition is a bead plus a `SendMessage` to the holder, and a correction is a
  `SendMessage` checked as received, or a bead when it was not. A hold sent as a message
  arrived a minute after the commit it was meant to hold (owner, 2026-11-09).
