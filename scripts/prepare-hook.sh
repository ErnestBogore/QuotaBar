#!/bin/sh
set -eu

profile="${1:-debug}"
case "$profile" in
  debug)
    cargo build -p quotabar-hook
    source_binary="target/debug/quotabar-hook"
    ;;
  release)
    cargo build -p quotabar-hook --release
    source_binary="target/release/quotabar-hook"
    ;;
  *)
    echo "usage: $0 [debug|release]" >&2
    exit 2
    ;;
esac

host_triple="$(rustc -vV | awk '/^host:/ {print $2}')"
destination="src-tauri/binaries/quotabar-hook-${host_triple}"
mkdir -p src-tauri/binaries
cp "$source_binary" "$destination"
chmod 755 "$destination"
echo "Prepared $destination"

