# Releasing this crate

## Prerequisites

* Ensure you have a GPG keypair created that you can use to sign the
  release tag.
* Install `toml` with `cargo install toml`.

## Releasing

1. Run `./prepare-release.sh NEW_VERSION` (substituting the new desired
   version for `NEW_VERSION`).
2. Go to the GitHub releases page for this repo.  There should be a
   draft release waiting.  Edit it, set the title to the new version,
   point it to the new tag that was just pushed, and scan through the
   release notes to ensure they seem sane.  When you click the Publish
   button, GitHub Actions will build the new release and publish it to
   crates.io.

## TODO

* Use the GH API to do step #2 above.
