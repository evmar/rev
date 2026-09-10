#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 PROJECT_PATH" >&2
    exit 1
fi

REV_PROJECT=$(cd "$1" && pwd)
export REV_PROJECT
cd "$(dirname "$0")"

exec cargo watch -w src -w Cargo.toml -w Cargo.lock \
    -x 'run -- --project "$REV_PROJECT" web --dev'
