.PHONY: all clean test

SRC_FILES := $(wildcard *.asm)
GB_FILES := $(patsubst %.asm, bin/%.gb, $(SRC_FILES))

all: $(GB_FILES)

bin/%.gb: %.asm
	@mkdir -p $(dir $@)
	rgbasm -o $@ $<
	rgblink -o $@ $@
	rgbfix -v -p 0 $@

test: all
	cd test_runner && cargo run --quiet

clean:
	rm -rf bin
