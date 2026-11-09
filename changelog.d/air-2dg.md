Fixed a pinned `air` that could not write its delegated event said nothing, so a lane batch
  read the missing event as a product failure with no cause. It now prints
  `air: delegated event not written: <cause>` on stderr and runs the command as before.
