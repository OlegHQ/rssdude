PREFIX ?= $(HOME)/.cargo
BIN    := $(PREFIX)/bin/rssdude

# On macOS, native_model needs libiconv from Homebrew (keg-only).
# On Linux, libiconv is part of glibc — no extra path needed.
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
  LIBICONV ?= /opt/homebrew/opt/libiconv/lib
  export LIBRARY_PATH := $(LIBICONV)$(if $(LIBRARY_PATH),:$(LIBRARY_PATH))
endif

SERVICE := rssdude
BIND    ?= 0.0.0.0:8484

.PHONY: build install uninstall release run check clippy test fmt clean up down restart status logs reinstall-service

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

# --- service management via smdctl -----------------------------------------
# Idempotent: starts the service if already registered, otherwise creates it.
# Override the bind address with: make up BIND=127.0.0.1:8484

up: install
	@smdctl status $(SERVICE) >/dev/null 2>&1 \
		&& smdctl start $(SERVICE) \
		|| smdctl run $(SERVICE) -description "rssdude RSS feed server" \
			-restart always -- $(BIN) serve --bind $(BIND)

down:
	smdctl stop $(SERVICE)

restart:
	smdctl restart $(SERVICE)

status:
	smdctl status $(SERVICE)

logs:
	smdctl logs -f $(SERVICE)

reinstall-service:
	-smdctl rm -f $(SERVICE)
	$(MAKE) up
