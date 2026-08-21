#!/bin/sh
set -eu

arm_target="aarch64-apple-darwin"
intel_target="x86_64-apple-darwin"

rustup target add "$arm_target" "$intel_target"
cargo build -p quotabar-hook --release --target "$arm_target"
cargo build -p quotabar-hook --release --target "$intel_target"

destination="src-tauri/binaries/quotabar-hook-universal-apple-darwin"
mkdir -p src-tauri/binaries
lipo -create \
  "target/$arm_target/release/quotabar-hook" \
  "target/$intel_target/release/quotabar-hook" \
  -output "$destination"
chmod 755 "$destination"
echo "Prepared $destination"
