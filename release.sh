#!/bin/bash

set -e

token="$1"
new_version=$(awk '/^version = /{ print $3; exit; }' espmonitor/Cargo.toml | sed -e 's/"//g')

[ "token" ] || {
    echo "Usage: $0 CRATES_IO_TOKEN" >&2
    exit 1
}
[ "$new_version" ] || {
    echo "Unable to determine new version" >&2
    exit 1
}

echo "Publishing espmonitor..."
pushd espmonitor
cargo publish --token $token
popd

echo "Waiting for espmonitor $new_version to become available on crates.io"
wait=60
max_version=''
while [ $wait -gt 0 -a "$max_version" != "$new_version" ]; do
    sleep 1
    max_version=$(curl -s -f https://crates.io/api/v1/crates/espmonitor | jq -r .crate.max_version)
    wait=$((wait-1))
done
if [ "$max_version" == "$new_version" ]; then
    sleep 5
    echo "New version $max_version published"
else
    echo "New version $new_version failed to show up; latest version is currently $max_version" >&2
    exit 1
fi

echo "Publishing cargo-espmonitor..."
pushd cargo-espmonitor
cargo publish --token $token
popd
