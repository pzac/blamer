#!/bin/sh

set -e

if [ -z "$1" ]; then
  echo "Usage: $0 <tag>"
  exit 1
fi

TAG="$1"
ARCHIVE="blamer-${TAG}.tar.gz"

tar czf "$ARCHIVE" \
  --exclude="./.git" \
  --exclude="./.jj" \
  --exclude="./target" \
  --exclude="./.claude" \
  --exclude="./*.tar.gz" \
  .

shasum -a 256 "$ARCHIVE"
