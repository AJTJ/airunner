Fixed `air ui`'s `/api/feed` with no offset answered the oldest lines of its history, so the
  page's first load showed a week-old slice as "newest" and caught up 200 lines a poll. It now
  tails today's event file for the newest 200 items, and its offset is day-scoped
  (`<day>:<byte position>`): a later call reads the rest of that day and each newer day whole.
