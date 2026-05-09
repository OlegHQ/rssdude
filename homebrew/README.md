# Homebrew tap

Files staged here belong in the [`OlegHQ/homebrew-tap`](https://github.com/OlegHQ/homebrew-tap) repo.
After a release, copy `Formula/rssdude.rb` into that repo with the real SHA256s filled in.

## End-user install

```bash
brew tap OlegHQ/tap
brew install rssdude
```

## Release workflow

1. Push a `v*` tag on this repo. `.github/workflows/release.yml` builds
   `aarch64-apple-darwin` and `x86_64-apple-darwin` binaries and attaches them
   to the GitHub release.
2. Grab the SHA256s from the `*.sha256` files in the release.
3. Update `Formula/rssdude.rb` in the tap repo with the new `version` and SHAs,
   commit, push.
