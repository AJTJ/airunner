Fixed `air land` ended without the lane's next step when the landing carried no bead, such as
  the coordinator's branch or a journal-only one, and the lane polled `air status` instead.
  Every landing that moves main now ends with the batch-ready branches and `next: air batch
  cut`, or `nothing is batch-ready`.
