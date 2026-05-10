# Install

## Apple Silicon mac (Homebrew)

```bash
brew tap OlegHQ/tap
brew install rssdude
```

That's the whole thing. The tap pulls a prebuilt `aarch64-apple-darwin` binary attached to the latest GitHub release.

## From source — macOS

```bash
brew install libiconv     # keg-only, the linker needs it
git clone https://github.com/OlegHQ/rssdude
cd rssdude
make install              # cargo install --path . into ~/.cargo/bin
```

The `LIBRARY_PATH=/opt/homebrew/opt/libiconv/lib` is set automatically by the Makefile. If you build with bare `cargo build` outside the Makefile, you'll need to export it yourself.

## From source — Linux

```bash
git clone https://github.com/OlegHQ/rssdude
cd rssdude
cargo install --path .
```

No `libiconv` dance needed on Linux — glibc provides it.

## Make targets

The Makefile is a tiny convenience layer over `cargo`. The targets you'll actually use:

| Target | What it does |
|--------|--------------|
| `make build`     | Release build into `target/release/rssdude` |
| `make install`   | `cargo install` to `$(PREFIX)/bin` (default `~/.cargo`) |
| `make uninstall` | Remove the installed binary |
| `make test`      | Unit + snapshot tests |
| `make clippy`    | Lint with `-D warnings` |
| `make run ARGS="items --unread"` | `cargo run` passthrough |

Pass `make install PREFIX=/usr/local` if you'd rather it land somewhere else.

## Verify

```bash
rssdude --version
rssdude --help
```

If `rssdude` isn't found, check that `~/.cargo/bin` is on your `PATH`.

## Upgrading

Homebrew:

```bash
brew update && brew upgrade rssdude
```

From source:

```bash
cd rssdude && git pull && make install
```

The database format is forward-compatible — your existing `~/.rssdude/rssdude.redb` keeps working across upgrades. If a future version ever needs a migration, the binary handles it on first run.

## Uninstall

```bash
make uninstall              # removes the binary
rm -rf ~/.rssdude            # removes the database & config — only if you really want to
```
