PREFIX   ?= $(HOME)/.cargo
LIBICONV ?= /opt/homebrew/opt/libiconv/lib
BIN      := $(PREFIX)/bin/rssdude

.PHONY: build install uninstall release run check clippy test fmt clean

build:
	LIBRARY_PATH=$(LIBICONV) cargo build --release

install:
	LIBRARY_PATH=$(LIBICONV) cargo install --path . --root $(PREFIX) --force
	@echo
	@echo "✅ rssdude installed to $(BIN)"
	@echo "   make sure $(PREFIX)/bin is on your PATH"

uninstall:
	cargo uninstall --root $(PREFIX) rssdude

release: build

run:
	LIBRARY_PATH=$(LIBICONV) cargo run --release -- $(ARGS)

check:
	LIBRARY_PATH=$(LIBICONV) cargo check --all-targets

clippy:
	LIBRARY_PATH=$(LIBICONV) cargo clippy --all-targets -- -D warnings

test:
	LIBRARY_PATH=$(LIBICONV) cargo test

fmt:
	cargo fmt --all

clean:
	cargo clean
