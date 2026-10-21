# Builds the draft test ROMs with RGBDS (1.0 is tested):
#
#   make            all of them
#   make <name>.gb  one

ROMS := $(patsubst %.asm,%.gb,$(wildcard *.asm))

all: $(ROMS)

%.gb: %.asm common.inc
	rgbasm -o $*.o $<
	rgblink -o $@ $*.o
	rgbfix -v -p 0xFF $@
	rm -f $*.o

clean:
	rm -f *.gb *.o

.PHONY: all clean
