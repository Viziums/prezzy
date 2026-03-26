# Release Checklist

## Before release

- [ ] All tests pass: `cargo test --all-targets`
- [ ] Zero clippy warnings: `cargo clippy --all-targets`
- [ ] Version bumped in `Cargo.toml`
- [ ] README is up to date
- [ ] CHANGELOG updated (if exists)

## Release steps

1. Bump version in `Cargo.toml`
2. Commit: `git commit -am "chore: bump version to X.Y.Z"`
3. Tag: `git tag vX.Y.Z`
4. Push: `git push origin main --tags`
5. Wait for GitHub Actions to build all 6 targets and create the release
6. Publish to crates.io: `cargo publish`
7. Update Homebrew formula SHA256 hashes
8. Update npm package version and publish: `cd packaging/npm && npm publish`

## Post-release

- [ ] Verify `cargo install prezzy` works
- [ ] Verify `brew install prezzy-cli/tap/prezzy` works
- [ ] Verify GitHub Release has all 6 binaries
- [ ] Announce on social media
