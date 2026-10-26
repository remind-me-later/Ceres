# The gate-level oracle: dmg-sim, the simulation of the DMG-CPU B netlist,
# on Icarus Verilog, with our probes compiled in (see README.md).
#
#   make            build dmg-sim from its submodule, with the probes
#   make run        simulate SECS seconds after the reset, trace the pins
#   make smoke      33 ms from power-on with dmg-sim's quickboot ROM: the
#                   pin trace must have the hash in smoke.sha256
#   make phases     simulate as make run, and record the signals the
#                   readable model is checked against (PHASES)
#   make model      analyse the readable model (rtl/), its board (board/)
#                   and its checks
#   make run-model  run ROM on the readable model, on its own, for SECS
#                   seconds after the reset (SCREEN: the last frame, as a
#                   PGM picture)
#   make check      check the readable model against the recorded signals
#                   of every test in build/runs, JOBS at a time (see
#                   run-tests.sh)
#
# Variables:
#   SECS     simulated seconds after the chip's power-on reset (31.2 ms)
#   ROM      cartridge ROM (none, MBC1 or MBC5 from its header; "" for none)
#   BOOTROM  boot ROM image
#   TRACE    file for the changes of the pins ("" for none)
#   PHASES   file for the signals of each phase (make phases)
#   ORACLE   recorded signals to check against ("" for every test's)
#   SCREEN   file for the last frame on the LCD (make run-model)
#   JOBS     checks at a time (make check)

SECS    ?= 0.01
ROM     ?=
BOOTROM ?= ../external/gb-bootroms/bin/dmg.bin
TRACE   ?= trace.txt
PHASES  ?= phases.txt
ORACLE  ?=
SCREEN  ?=
JOBS    ?= 6

BUILD   := build
DMG_SIM := ../external/reference-implementations/dmg-sim
SIM     := $(BUILD)/dmg-sim
VVP     := $(SIM)/dmg_cpu_b_gameboy.vvp
PROBES  := oracle/probe_pins.sv oracle/probe_results.sv oracle/probe_phases.sv

QUICKBOOT := $(DMG_SIM)/boot/quickboot.bin
FASTBOOT  := $(BUILD)/fastboot.bin
TINYBOOT  := $(BUILD)/tinyboot.bin
TINYBOOT_CGB := $(BUILD)/tinyboot_cgb.bin
APU_ROM   := $(BUILD)/apu.gb
CPU_ROM   := $(BUILD)/cpu.gb
SPRITES_ROM := $(BUILD)/sprites.gb
STAT_ROM  := $(BUILD)/stat_vblank_end.gb
LCD_OFF_ROM := $(BUILD)/lcd_off_line_start.gb
WINDOW_ROM := $(BUILD)/window_line0_sprite.gb

abs = $(abspath $(1))

.PHONY: all run phases smoke model run-model check clean

all: $(VVP) $(FASTBOOT) $(TINYBOOT) $(TINYBOOT_CGB) $(APU_ROM) $(CPU_ROM) $(SPRITES_ROM) $(STAT_ROM) $(LCD_OFF_ROM) $(WINDOW_ROM)

# A boot ROM that only turns the LCD on and goes to 0100, for the checks
# of the readable model, which do not need the boot ROM's timing:
#   0000  LD SP,FFFE / LD A,91 / LDH (40),A / LD A,01 / JP 00FE
#   00FE  LDH (50),A
$(FASTBOOT):
	mkdir -p $(BUILD)
	{ printf '\061\376\377\076\221\340\100\076\001\303\376\000'; \
	  head -c 242 /dev/zero; printf '\340\120'; } >$@

# A boot ROM that leaves the machine as dmg-sim's quickboot does, in a ninth
# of the time (boot/tinyboot.asm), for the runs of the readable model.
$(TINYBOOT): boot/tinyboot.asm
	mkdir -p $(BUILD)
	rgbasm -o $(BUILD)/tinyboot.o $<
	rgblink -x -o $@ $(BUILD)/tinyboot.o
	rm -f $(BUILD)/tinyboot.o

# A boot ROM that leaves the CGB as the CGB-E's boot ROM does, in a
# hundredth of the time (boot/tinyboot_cgb.asm), for the runs of the CGB's
# model.
$(TINYBOOT_CGB): boot/tinyboot_cgb.asm
	mkdir -p $(BUILD)
	rgbasm -o $(BUILD)/tinyboot_cgb.o $<
	rgblink -x -o $@ $(BUILD)/tinyboot_cgb.o
	rm -f $(BUILD)/tinyboot_cgb.o

# The stimuli of the APU, the CPU core and the PPU's sprites, for the checks too
# (check/apu.asm, check/cpu.asm, check/sprites.asm), and of STAT at the end
# of VBlank, of the LCD turned off as a line begins and of the window on line
# 0, for compare-ceres.sh (check/stat_vblank_end.asm,
# check/lcd_off_line_start.asm, check/window_line0_sprite.asm).
$(BUILD)/%.gb: check/%.asm
	mkdir -p $(BUILD)
	rgbasm -o $(BUILD)/$*.o $<
	rgblink -o $@ $(BUILD)/$*.o
	rgbfix -v -p 0 $@

# dmg-sim is copied from its submodule, which stays clean. Icarus 13 has no
# string method tolower, which dmg-sim only uses for event files.
$(VVP): $(PROBES)
	rm -rf $(SIM)
	mkdir -p $(BUILD)
	cp -r $(DMG_SIM) $(SIM)
	sed -i 's|cmd = cmd.tolower();|// cmd = cmd.tolower();|' $(SIM)/dmg_cpu_b_gameboy.sv
	$(MAKE) -C $(SIM) dmg_cpu_b_gameboy.vvp \
		IVERILOG_FLAGS="-g2012 -gspecify -f timescale.f -pfileline=1 -gsupported-assertions $(call abs,$(PROBES))"

run: $(VVP)
	vvp -N $(VVP) -none +BOOTROM=$(call abs,$(BOOTROM)) $(if $(ROM),+ROM=$(call abs,$(ROM))) \
		+SECS=$(SECS) $(if $(TRACE),+PINS_FILE=$(call abs,$(TRACE)))

phases: $(VVP)
	vvp -N $(VVP) -none +BOOTROM=$(call abs,$(BOOTROM)) $(if $(ROM),+ROM=$(call abs,$(ROM))) \
		+SECS=$(SECS) +PHASES_FILE=$(call abs,$(PHASES))

# The readable model, in the order nvc analyses it, and its checks.
MODEL  := rtl/clocking.vhd rtl/revisions.vhd rtl/clocks.vhd rtl/divider.vhd rtl/timer.vhd rtl/interrupts.vhd \
          rtl/serial.vhd rtl/joypad.vhd rtl/test_mode.vhd rtl/boot_rom.vhd rtl/cgb_mode.vhd rtl/speed_switch.vhd rtl/high_ram.vhd \
          rtl/wram_bank.vhd \
          rtl/external_bus.vhd rtl/dma.vhd rtl/lcd_control.vhd rtl/lcd_timing.vhd \
          rtl/oam_scan.vhd rtl/sprite_store.vhd rtl/oam.vhd rtl/render.vhd rtl/colour_palettes.vhd rtl/vram_bus.vhd \
          rtl/video_ram.vhd \
          rtl/apu_control.vhd rtl/apu_ch1.vhd rtl/apu_ch2.vhd rtl/apu_ch3.vhd rtl/apu_ch4.vhd \
          rtl/sm83_decoder.vhd rtl/sm83_control.vhd rtl/sm83_sequencer.vhd rtl/sm83_interrupts.vhd \
          rtl/sm83_registers.vhd rtl/sm83_idu.vhd rtl/sm83_alu.vhd rtl/sm83.vhd \
          board/keeper.vhd rtl/cgb_cpu.vhd
BOARD  := board/cartridge.vhd board/gameboy.vhd board/gameboy_tb.vhd
# The checks against dmg-sim are the DMG's (branch hdl): they do not apply
# to the CGB's chip.
CHECKS :=
TBS    := chip_tb
WORK   := $(BUILD)/model
NVC    := nvc --std=2008 -H 64m --work=$(abspath $(WORK))/work

model: $(WORK)/analysed

$(WORK)/analysed: $(MODEL) $(BOARD) $(CHECKS)
	mkdir -p $(WORK)
	$(NVC) -a $(call abs,$(MODEL) $(BOARD) $(CHECKS))
	touch $@

run-model: $(WORK)/analysed
	$(NVC) -e --jit gameboy_tb -gBOOTROM=$(call abs,$(BOOTROM)) $(if $(ROM),-gROM=$(call abs,$(ROM))) \
		-gSECS=$(SECS) $(if $(SCREEN),-gSCREEN=$(call abs,$(SCREEN))) -r

# Each testbench on each test's recorded signals (build/runs and
# build/runs-fast, see run-tests.sh).
ORACLES = $(if $(ORACLE),$(ORACLE),$(shell find $(BUILD)/runs $(BUILD)/runs-fast -name phases.txt 2>/dev/null | sort))

# Each check prints its report in one piece, when it ends.
check: $(WORK)/analysed $(FASTBOOT)
	@printf '%s\n' $(abspath $(ORACLES)) | xargs -P $(JOBS) -I{} sh -c '\
		o={}; \
		case $$o in */runs-fast/*) boot=$(call abs,$(FASTBOOT));; *) boot=$(call abs,$(QUICKBOOT));; esac; \
		for tb in $(TBS); do \
			report=$$($(NVC) -e --jit $$tb -gORACLE=$$o -gBOOTROM=$$boot -r 2>&1 \
				| grep -v "^   \|FINISH called\|metavalue detected"); \
			printf "%s %s\n%s\n" $$tb "$${o#$(abspath $(BUILD))/}" "$$report"; \
		done'

# The changes at one time are sorted, so that the hash does not depend on
# the order Icarus runs the probes in.
smoke:
	$(MAKE) run SECS=0.0018 BOOTROM=$(QUICKBOOT) TRACE=$(BUILD)/smoke.txt
	LC_ALL=C sort $(BUILD)/smoke.txt | sha256sum -c smoke.sha256

clean:
	rm -rf $(BUILD)
