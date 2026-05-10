PREFIX ?= $(HOME)/.cargo
BIN    := $(PREFIX)/bin/rssdude

# On macOS, native_model needs libiconv from Homebrew (keg-only).
# On Linux, libiconv is part of glibc — no extra path needed.
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
  LIBICONV ?= /opt/homebrew/opt/libiconv/lib
  export LIBRARY_PATH := $(LIBICONV)$(if $(LIBRARY_PATH),:$(LIBRARY_PATH))
endif

.PHONY: build install uninstall release run check clippy test fmt clean

build:
	cargo build --release

install:
	cargo install --path . --root $(PREFIX) --force
	@echo
	@echo "✅ rssdude installed to $(BIN)"
	@echo "   make sure $(PREFIX)/bin is on your PATH"

uninstall:
	cargo uninstall --root $(PREFIX) rssdude

release: build

run:
	cargo run --release -- $(ARGS)

check:
	cargo check --all-targets

clippy:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test

fmt:
	cargo fmt --all

clean:
	cargo clean
