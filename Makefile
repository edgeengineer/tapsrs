.PHONY: all build test clean

# Variables
CARGO := rustup run stable -- cargo
TARGET_DIR ?= target
HEADER_FILE := $(TARGET_DIR)/taps.h

all: build

build: $(HEADER_FILE)

$(HEADER_FILE): src/lib.rs build.rs cbindgen.toml
	$(CARGO) build

test:
	$(CARGO) test

run-test-c: build
	$(CC) test.c -L$(TARGET_DIR)/debug -ltapsrs -o $(TARGET_DIR)/test -framework Security -framework CoreFoundation
	$(TARGET_DIR)/test

TARGETS := \
	aarch64-apple-ios \
	x86_64-apple-darwin \
	aarch64-apple-darwin \
	x86_64-unknown-linux-gnu \
	aarch64-unknown-linux-gnu \
	x86_64-pc-windows-msvc \
	aarch64-pc-windows-msvc \
	aarch64-linux-android

.PHONY: cross-compile
cross-compile: $(foreach target,$(TARGETS),build-$(target))

$(foreach target,$(TARGETS),build-$(target)):
	$(CARGO) build --target $(patsubst build-%,%,$@) --release

clean:
	$(CARGO) clean
	rm -f $(TARGET_DIR)/test $(HEADER_FILE) 