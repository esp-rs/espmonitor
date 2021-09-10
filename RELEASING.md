# Releasing this crate

1. Bump the version in `espmonitor/Cargo.toml` and
   `cargo-espmonitor/Cargo.toml`.  Also bump the `espmonitor` dependency
   version in `cargo-espmonitor/Cargo.toml`.
2. Run `cargo build` to update `Cargo.lock'.`
3. Commit these changes.
4. Tag the commit with the new version (prefixed with a "v"; e.g.
   `v0.3.1`).  If you are into that sort of thing, sign the tag with
   your GPG key.
5. Push the version bump commit and the tag.
6. Go to the GitHub releases page for this repo, and create a release
   against the tag you just pushed.
7. Once the release is created, GitHub Actions will build the new
   release and publish it to crates.io.

Alternative: if you don't GPG-sign your tag, you can also just create
the tag as a part of creating the GH release.
