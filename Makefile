BINARY := axe
INSTALL_PATH := /usr/local/bin/$(BINARY)

.PHONY: build install clean fmt clippy test deny check

build:
	cargo build --release

install: build
	rm -f $(INSTALL_PATH)
	cp target/release/$(BINARY) $(INSTALL_PATH)

clean:
	cargo clean

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

deny:
	cargo deny check

check: fmt clippy test
