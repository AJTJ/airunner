Added `air ui [--port N]`: a read-only browser view of the fleet on 127.0.0.1, port 7311 by
  default (`--port 0` takes a free one). In this bead it is the skeleton: the page is a
  placeholder, and the five endpoints (`/api/fleet`, `/api/lane`, `/api/beads`, `/api/feed`,
  `/api/attention`) answer their final shapes with the sessions filled and the rest empty. The
  ledger is opened read-only and the server's working directory is `/`, so it writes nothing
  and reads as no worker's process.
