# Releasing this crate

1. Run `./prepare-release.sh NEW_VERSION` (substituting the new desired
   version for `NEW_VERSION`).
2. Go to the GitHub releases page for this repo.  There should be a
   draft release waiting.  Edit it, set the title to the new version,
   and point it to the new tag that was just pushed.
3. Once the release is created, GitHub Actions will build the new
   release and publish it to crates.io.
