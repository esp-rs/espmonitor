#!/bin/bash

set -e

new_version="$1"

[ "$new_version" ] || {
    echo "Usage: $0 NEW_VERSION" >&2
    exit 1
}

toml_set() {
    local file="$1"
    local query="$2"
    local value="$3"

    [ "$value" ] || {
        echo "Usage: toml_set FILE QUERY VALUE" >&2
        return 1
    }

    toml set "$file" "$query" "$value" > "$file.new"
    mv "$file.new" "$file"
}

set_version() {
    local version="$1"

    [ "$version" ] || {
        echo "Usage: set_version VERSION" >&2
        return 1
    }

    toml_set espmonitor/Cargo.toml package.version "$version"
    toml_set cargo-espmonitor/Cargo.toml dependencies.espmonitor.version "^$version"
    toml_set cargo-espmonitor/Cargo.toml package.version "$version"
}

# The does not work on alpha/rc/etc. versions, just x.y.z versions
new_patch_version() {
    local old_version="$1"

    [ "$old_version" ] || {
        echo "Usage: new_patch_version OLD_VERSION" >&2
        exit 1
    }

    echo "$old_version" | awk -F. '{ ++$3; printf "%d.%d.%d", $1, $2, $3 }'
}

if ! type -p toml; then
    echo "Installing toml-cli..."
    cargo install toml-cli
fi

set_version "$new_version"
cargo update --package espmonitor
git commit -a -m "Bump to $new_version"
git tag -s "v$new_version" -m "$new_version"

dev_version=$(new_patch_version "$new_version")-alpha.1
set_version "$dev_version"
cargo update --package espmonitor
git commit -a -m "Bump to $dev_version"

git push --tags origin "$(git rev-parse --abbrev-ref HEAD)"

echo 'Now visit:'
echo
echo 'https://github.com/esp-rs/espmonitor/releases/'
echo
echo 'Rename the draft release, point the release to the newly-created tag, and'
echo 'publish the release.  GH Actions will take care of publishing to crates.io'
