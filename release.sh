#!/bin/bash

set -e

token="$1"

echo "Publishing espmonitor..."
pushd espmonitor
cargo publish --token $token
popd

echo "Waiting for espmonitor to become available on crates.io"
sleep 60

echo "Publishing cargo-espmonitor..."
pushd cargo-espmonitor
cargo publish --token $token
popd
