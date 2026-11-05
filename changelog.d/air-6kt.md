Fixed `scripts/changelog-assemble.sh` folded a leading blank line under the first entry of a
  heading on mawk (Debian's awk), which made every verify on the Linux pod red on the changelog
  tests; the key is now tested before the element is referenced.
