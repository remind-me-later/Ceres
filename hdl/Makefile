# Gate-level VHDL model of the Game Boy (see README.md).
#
#   make                 generate the VHDL from the submodules and analyse it
#   make run             simulate TIME of a Game Boy, write the pin trace
#   make SIM=ghdl run    the same with GHDL instead of nvc
#
# Variables:
#   TIME     simulated time, e.g. "40 ms" (the chip leaves reset after 31.2 ms)
#   ROM      cartridge ROM without memory bank controller ("" for none)
#   TRACE    file for the changes of the pins
#   BOOTROM  boot ROM image

SIM     ?= nvc
TIME    ?= 40 ms
ROM     ?=
TRACE   ?= trace.txt
BOOTROM ?= ../external/gb-bootroms/bin/dmg.bin

BUILD := build
GEN   := $(BUILD)/gen
WORK  := $(BUILD)/$(SIM)

GENERATED := $(GEN)/cells.vhd $(GEN)/sm83.vhd $(GEN)/dmg_cpu_b.vhd
SOURCES   := support/verilog.vhd gate/timing.vhd $(GEN)/cells.vhd gate/cells.vhd \
             $(GEN)/sm83.vhd $(GEN)/dmg_cpu_b.vhd tb/gameboy_tb.vhd
GENERATOR := $(wildcard ../ceres-hdlgen/src/*.rs ../ceres-hdlgen/src/*/*.rs)

NVC  := nvc -M 4g --std=2008
GHDL := ghdl

# Absolute paths, since the simulators run in $(WORK).
abs = $(abspath $(1))
rom_generic = $(if $(ROM),-gROM_FILE=$(call abs,$(ROM)))

.PHONY: all gen run clean

all: $(WORK)/analysed

gen: $(GENERATED)

$(GENERATED) &: $(GENERATOR)
	cargo run --release -p ceres-hdlgen -- --out $(GEN)

$(BUILD)/nvc/analysed: $(SOURCES)
	mkdir -p $(BUILD)/nvc
	cd $(BUILD)/nvc && $(NVC) -a $(call abs,$(SOURCES))
	touch $@

$(BUILD)/ghdl/analysed: $(SOURCES)
	mkdir -p $(BUILD)/ghdl
	cd $(BUILD)/ghdl && $(GHDL) -a --std=08 $(call abs,$(SOURCES))
	touch $@

run: $(WORK)/analysed
	ln -sf $(call abs,$(BOOTROM)) $(WORK)/bootrom.bin
ifeq ($(SIM),nvc)
	cd $(WORK) && $(NVC) -e --jit gameboy_tb -gSIM_TIME='$(TIME)' $(rom_generic) \
		-gTRACE_FILE=$(call abs,$(TRACE)) -r
else
	cd $(WORK) && $(GHDL) --elab-run --std=08 gameboy_tb -gSIM_TIME='$(TIME)' $(rom_generic) \
		-gTRACE_FILE=$(call abs,$(TRACE))
endif

clean:
	rm -rf $(BUILD)
