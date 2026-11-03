Changed Surface notices are one file each under `crates/cli/surface.d/<ordinal>-<id>.rs`, folded
  into `install::SURFACE` by `build.rs`, so two beads adding a notice never conflict at the same
  line. A missing or empty directory fails the build; a malformed fragment is a rustc error
  naming the file.
