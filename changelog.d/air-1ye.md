Added On a Mac the launchers run the session under `caffeinate -i`, so an hour-long command no
  longer idle-sleeps the machine (three hours lost at an adopter on 2026-10-29); the assertion
  ends with the session. `air status` prints `idle sleep: <n> min` when `pmset -g` reports
  under ten minutes.
