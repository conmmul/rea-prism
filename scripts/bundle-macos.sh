#!/bin/sh
# Build a double-clickable macOS app bundle around the release binary.
set -eu

cd "$(dirname "$0")/.."
cargo build --release

APP="dist/Prism.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp target/release/rea-prism "$APP/Contents/MacOS/Prism"

cat > "$APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>              <string>Prism</string>
  <key>CFBundleDisplayName</key>       <string>Prism</string>
  <key>CFBundleExecutable</key>        <string>Prism</string>
  <key>CFBundleIdentifier</key>        <string>io.github.conmmul.rea-prism</string>
  <key>CFBundlePackageType</key>       <string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key>           <string>1</string>
  <key>LSMinimumSystemVersion</key>    <string>11.0</string>
  <key>NSHighResolutionCapable</key>   <true/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key> <string>Prism document</string>
      <key>CFBundleTypeRole</key> <string>Editor</string>
      <key>CFBundleTypeExtensions</key>
      <array><string>prism</string><string>prismt</string><string>pzfx</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST

echo "APPL????" > "$APP/Contents/PkgInfo"
echo "built $APP"
