.PHONY: all test clean

all:
	$(MAKE) -C ceres_core/bootroms
	cargo build --release

test:
	$(MAKE) -C test
	cd test && make test

clean:
	$(MAKE) -C ceres_core/bootroms clean
	cargo clean
