#!/bin/sh
set -eu
cd "$(dirname "$0")"

MANIFEST=dev.skepy.wyre.yaml
GENERATOR=https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/74697c75b630d7330e77250fc13cb5ea688d9479/cargo/flatpak-cargo-generator.py
GENERATOR_SHA256=0a2db6be87d75910facef28ab46d4d6460802e8419ab850d0caa6a364d26b380

# writes the source lists $2... to $1, each crate once, with the cargo config
# renamed to config.toml so cargo doesn't warn
merge() {
    python3 - "$@" <<'EOF'
import json, sys
sources = {}
for path in sys.argv[2:]:
    for source in json.load(open(path)):
        if source.get("dest-filename") == "config":
            source["dest-filename"] = "config.toml"
        sources.setdefault((source.get("dest"), source.get("dest-filename")), source)
open(sys.argv[1], "w").write(json.dumps(list(sources.values()), indent=4) + "\n")
EOF
}

# the manifest's value for key $1, without quotes or spaces
value() {
    sed -n "s/^$1: *//p" "$MANIFEST" | tr -d "\"' \r"
}

# the rust-stable extension for the freedesktop SDK the manifest's SDK is
# built on, as its standard library depends on the exact Rust version
sdk=$(value sdk)
branch=$(value runtime-version)
metadata=$(flatpak info --show-metadata "$sdk" "$branch")
# BuildStream-built SDKs, like KDE's, write "key = value"
version=$(printf '%s\n' "$metadata" |
    sed -n '/^\[Extension org.freedesktop.Sdk.Extension\]/,/^\[/s/^versions* *= *\([^;]*\).*/\1/p')
[ -n "$version" ] || { echo "$sdk//$branch has no SDK extension version" >&2; exit 1; }
extension=$(flatpak info --show-location org.freedesktop.Sdk.Extension.rust-stable "$version")

bpf_linker=$(sed -n 's|^ *url: \(https://static.crates.io/crates/bpf-linker/.*\)|\1|p' "$MANIFEST" | tr -d '\r')
flatbox=$(sed -n '/flatbox\.git/,/tag:/s/^ *tag: *//p' "$MANIFEST" | tr -d "\"' \r")

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/bpf-linker" "$tmp/flatbox"

curl -fsSL -o "$tmp/generator.py" "$GENERATOR"
echo "$GENERATOR_SHA256  $tmp/generator.py" | sha256sum -c --quiet
curl -fsSL "$bpf_linker" | tar -xzf - -C "$tmp/bpf-linker" --strip-components=1
curl -fsSL -o "$tmp/flatbox/Cargo.lock" \
    "https://raw.githubusercontent.com/ilya-zlobintsev/flatbox/$flatbox/Cargo.lock"

python3 "$tmp/generator.py" ../Cargo.lock -o "$tmp/app.json"
python3 "$tmp/generator.py" "$extension/files/lib/rustlib/src/rust/library/Cargo.lock" -o "$tmp/std.json"
python3 "$tmp/generator.py" "$tmp/bpf-linker/Cargo.lock" -o "$tmp/bpf-linker.json"
python3 "$tmp/generator.py" "$tmp/flatbox/Cargo.lock" -o "$tmp/flatbox.json"

merge generated-sources.json "$tmp/app.json" "$tmp/std.json"
merge bpf-linker-sources.json "$tmp/bpf-linker.json"
merge flatbox-sources.json "$tmp/flatbox.json"