ARTIFACT := $(shell cargo pkgid |  rev | cut -d "/" -f1  | rev | cut -d "\#" -f1) # Try to determine the artifact name. If this does not work replace it with the explicit name.

install-dependencies:
	cargo vendor

build:
	docker run --rm -v $(PWD):/opt/project/ -w /opt/project/ pixix4/ev3dev-rust /bin/bash -c "cargo build --release --target armv5te-unknown-linux-gnueabi && /usr/bin/arm-linux-gnueabi-strip /opt/project/target/armv5te-unknown-linux-gnueabi/release/$(ARTIFACT)"

clean:
	cargo clean
