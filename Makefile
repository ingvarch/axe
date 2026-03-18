BINARY := axe
INSTALL_PATH := /usr/local/bin/$(BINARY)

.PHONY: build install clean

build:
	cargo build --release

install: build
	rm -f $(INSTALL_PATH)
	cp target/release/$(BINARY) $(INSTALL_PATH)

clean:
	cargo clean
