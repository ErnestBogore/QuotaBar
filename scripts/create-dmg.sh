#!/bin/bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: $0 /path/to/QuotaBar.app /path/to/QuotaBar.dmg" >&2
  exit 64
fi

app_path="$1"
output_path="$2"
repo_root="$(cd "$(dirname "$0")/.." && pwd)"
background_path="$repo_root/src-tauri/dmg-background.png"

if [[ ! -d "$app_path" || "$(basename "$app_path")" != "QuotaBar.app" ]]; then
  echo "QuotaBar.app was not found at: $app_path" >&2
  exit 66
fi

if [[ ! -f "$background_path" ]]; then
  echo "DMG background was not found at: $background_path" >&2
  exit 66
fi

work_dir="$(mktemp -d)"
staging_dir="$work_dir/staging"
writable_dmg="$work_dir/QuotaBar-writable.dmg"
device=""

cleanup() {
  if [[ -n "$device" ]]; then
    hdiutil detach "$device" -quiet || true
  fi
  rm -rf "$work_dir"
}
trap cleanup EXIT

mkdir -p "$staging_dir/.background" "$(dirname "$output_path")"
ditto "$app_path" "$staging_dir/QuotaBar.app"
ln -s /Applications "$staging_dir/Applications"
ditto "$background_path" "$staging_dir/.background/dmg-background.png"

hdiutil create \
  -volname "QuotaBar" \
  -srcfolder "$staging_dir" \
  -ov \
  -format UDRW \
  "$writable_dmg" >/dev/null

attach_output="$(hdiutil attach -readwrite -noverify -noautoopen -nobrowse "$writable_dmg")"
device="$(printf '%s\n' "$attach_output" | awk '/Apple_APFS|Apple_HFS/ {print $1; exit}')"

osascript <<'APPLESCRIPT'
tell application "Finder"
  tell disk "QuotaBar"
    open
    tell container window
      set current view to icon view
      set toolbar visible to false
      set statusbar visible to false
      set bounds to {100, 100, 760, 500}
      tell icon view options
        set arrangement to not arranged
        set icon size to 96
        set text size to 13
        set background picture to file ".background:dmg-background.png"
      end tell
      set position of item "QuotaBar.app" to {180, 220}
      set position of item "Applications" to {480, 220}
    end tell
    update without registering applications
    delay 2
    close
  end tell
end tell
APPLESCRIPT

sync
hdiutil detach "$device" -quiet
device=""
hdiutil convert "$writable_dmg" -format UDZO -ov -o "$output_path" >/dev/null

