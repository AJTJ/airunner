#!/bin/sh
# The project's check. Exits 0 when every case passes.
set -e
[ "$(sh greet.sh)" = "hello, world" ]
[ "$(sh greet.sh Ada)" = "hello, Ada" ]
echo "all tests passed"
