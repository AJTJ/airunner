Added The `air ui` page: one HTML file with no dependency, showing the fleet as one card per
  session (state, harness status, model, head, claims, the tool call in flight, the last
  assistant line, a context meter, the pane tail), the beads board in five columns, the lane
  (the verify in flight with a live counter, the current batch, the standing red with its
  untested-alone list, the last landings, the loop medians), attention and a filterable feed.
  Fast tiers poll every 2 s and beads every 10 s, slowing to 10 s in a hidden tab; a failed
  fetch shows one "server unreachable" line and keeps the last data.
