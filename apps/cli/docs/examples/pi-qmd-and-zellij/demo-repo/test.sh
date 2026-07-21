#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

test "$("$root/greet.sh")" = "Hello, World!"
test "$("$root/greet.sh" Ada)" = "Hello, Ada!"

echo "ok"
