-- SPDX-License-Identifier: MIT
--
-- Runs a ROM on the model of the Game Boy (gameboy.vhd), on its own, and
-- recognises how a test ROM reports its result, as ../oracle/probe_results.sv
-- does on dmg-sim:
--
--   mooneye      registers B to L at LD B,B
--   gbmicrotest  FF80 to FF82 in high RAM
--   blargg       "Passed" or "Failed" on the serial port
--
-- The run ends with the result, or after SECS seconds from the end of the
-- chip's reset (31.2 ms after power-on, as dmg-sim counts). The result goes
-- to RESULT_FILE ("pass ...", "fail ...", "timeout", or "done" without
-- TEST), what the serial port sends to SERIAL_FILE, and the last whole frame
-- on the LCD to SCREEN, as a PGM picture.
--
-- FRAMES gets every whole frame on the LCD, as ceres's example bus_trace
-- writes them (../compare-ceres.sh): "frame N", then 144 lines of 160 digits,
-- the pixels' two bits (0 white, 3 black).
--
-- PIXELS gets what the pixel pipeline does, a line each (../compare-pixels.sh,
-- as ceres's bus_trace writes them): "<LY> P <x> <t>" for a pixel the LCD
-- takes (column x) and "<LY> T <w|b> <t>" for a tile fetch that begins (the
-- window's or the background's) and "<LY> W <40|42|43|47|4B> <value> <t>"
-- for a change of LCDC, SCY, SCX, BGP or WX, t in half dots of the crystal.
--
-- BUS_TRACE gets a line per memory access of the CPU, as ceres's example
-- bus_trace writes them (../compare-ceres.sh): the M-cycle, counted from
-- the CPU's first access, R or W, the address and the byte, in hexadecimal.
--
-- CUTS gets what the blocks of the chip show each other, a line per change,
-- as ceres's example bus_trace writes them (../compare-cuts.sh): the
-- M-cycle, then IF, TIMA, DMA (1 while OAM DMA runs), LY or MODE (STAT's mode
-- bits) as a read would see them, OAMB and VRAMB (1 while the CPU cannot
-- read OAM, or VRAM), and DIV when the divider does not count on (a write
-- to DIV, STOP), its value in M-cycles; values in hexadecimal.
--
-- BLOCKS gets the divider, the timer, IF, OAM DMA, the line timing and the
-- OAM scan (the DMG's) at the end of every phase, from
-- the CPU's first access on, for the replay through ceres-netlist's
-- transcriptions of them (its tests/replay.rs): the phase that ends; the
-- address, RD and WR, and the data bus; DIV's counter, TIMA, TMA and TAC;
-- the timer's overflow flip-flops (int_timer, bit7_before), its clock and
-- its write strobes of TMA and TAC; the interrupt requests, the
-- acknowledges, IF's flags and its read latch, IE, and the requests that
-- rose in the phase (also those that fell again before its end); OAM DMA's
-- FF46, its flip-flops (requested, start_1, start_2, enabled, done, run)
-- and its count, OAM's DMA write strobe, and the DMA's clock dma_phi (high
-- in A to D, and while the CPU is halted); the clock as the phase's
-- closing edge leaves it, the LCD's reset and modes 0, 2 and 3; the line
-- timing's wuvu, vena, wosu, vclk, nype, line_153, first_line, mode1 and
-- lyc_match, LX, LY, LYC and STAT's enables; the byte a read takes, the
-- core's data bus in the middle of phase H; and the OAM scan's start_scan,
-- anel, scanning, ceno, byba, doba, not_saving, y_match and LCDC bit 2, its
-- index and count, the save strobes and the Y it reads (OAM's second even
-- latches); OAM's first even latches, and the sprite fetch's fetch_read and
-- fetch_hold; the clocks' inputs (CK1, /RESET, T1, T2, osc_ena, clk_ena,
-- mreq, write, test_reset, the divider's last bit), their latches and
-- resets (AFUR, ALEF, APUK, ADYK, TUBO's waiting, osc_stable, sys_reset,
-- cpu_sys_reset, reset2) and the clocks they make (clk_1mhz, clk_t4,
-- data_phase, exec_phase, pch_phase, cpu_wr_sync, cpu_ext_cs_en, PHI);
-- then render's flip-flops and latches (mode3_i, voga, px9, lyzu, lovy,
-- fetching, nyka, pory, pygo, started, sary, wy_latch, pyco, nunu, nopa,
-- in_window, rydy, sovy, puxa, nyze, paho, discard, ryfa, rene,
-- line_started, sobu, suda, busy, tobu, vonu, seba, tyfo), the pixel
-- counter, the fetcher's count, the fine count, the sprite fetch's step,
-- the window's column and line, the tile register, the low byte, the
-- background shifter, the sprite's bytes, the sprite shifter (its planes,
-- palette and priority), SCY, SCX, WY, WX, BGP, OBP0 and OBP1; what render
-- takes in (line_start, stop_scan, x_match, sprite_loaded, rendering,
-- VRAM's data, OAM's odd byte, the sprite's line) and gives out (its VRAM
-- address, CP, ST, LD); and the sprite store's scanned index, its fetched
-- flags, and its slots' X, lines and indexes; then the data pins, the
-- CPU core's non_vram_mreq, OAM DMA's dma_addr_ext and vram_to_oam, OAM's
-- wuje and maka, VRAM's /MCS, /MRD and /MWR, OAM's first odd latches and
-- its two data buses, and VRAM's address; then the bus side: the core's
-- mreq, read and write, /RD, /WR, /CS, the boot ROM's unmapped and
-- boot_sel, the address pins, the external bus's address and data latches,
-- the joypad's P1, read latch, debounce history and wake-up latch, its
-- input pins, and the serial port's SB, transfer, internal clock, half,
-- output, SCK and SIN, and count; whether the external bus's address
-- latches were open in the phase, for however short; then the CPU core:
-- its sequencer's latches and flip-flops (halt, stop, halt_delayed,
-- stop_delayed, pending_delayed, not_ready_1, not_ready_2, ime_state, ime,
-- no_nmi, no_nmi_1, entering, int_entry, sel_reg_w, cb_next, cb_flag), the
-- ALU's cond_fail, z7_master and z7_delayed, the opcode, IR's next value
-- and the M-cycle, the registers (AF with the flags as they show, BC, DE,
-- HL, WZ, SP, PC), the requests the interrupt logic latched, the core's
-- data bus, and the bits of the chip's data bus that were a strong 0 in
-- the phase, for however short (only those pull the core's bus down: not
-- the keeper's weak 0, nor an X where drivers fight, which the data bus's
-- field shows as 0); then the APU: its control's flip-flops and latches
-- (power, test, test_bit5, baza, ajer, calo, atep, cemo, cery, atyk, avok,
-- jeso, bara, caru, bylu), NR50 and NR51; channel 1's flip-flops and
-- latches (length_on, trigger, start, triggered, restart, restarted,
-- running, frozen, active, reload, wave, expired, hz64, tick, at_limit,
-- env_stopped, sweep_step, shifting, shift_done), NR10, the duty, NR12,
-- the frequency, its counter, the duty's step, the length, the envelope's
-- period, the volume, the sweep's count, its shifts, the shifted frequency
-- and the sum's augend and addend; channel 2's (as channel 1's, without
-- the sweep's), the duty, NR22, the frequency, the counter, the step, the
-- length, the period and the volume; channel 3's (dac_on, length_on,
-- trigger, start, triggered, restart, restarted, running, frozen,
-- playing, active, reload, wrapped, expired, busa, bano, fetch, azet), the
-- level, the frequency, the counter, the position, the length, the bit
-- lines, the sample's byte and wave RAM; channel 4's (as channel 2's, with
-- divided and joto for wave and reload), NR42, NR43, the divider, the
-- prescaler, the LFSR, the length, the period and the volume. BLOCKS needs
-- the channels (WITH_CHANNELS). Values in hexadecimal, bits as 0 or 1.
-- With BLOCKS_FROM_POWER_ON, from the power-on.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use std.textio.all;
use std.env.finish;

use work.clocking.all;
use work.high_ram_types.all;
use work.sm83_controls.all;
use work.revisions.all;
use work.sprite_slots.all;
use work.apu_wave.all;

entity gameboy_tb is
	generic (
		ROM         : string := "";
		BOOTROM     : string;
		SECS        : real;
		TEST        : string := "";
		RESULT_FILE : string := "";
		SERIAL_FILE : string := "";
		SCREEN      : string := "";
		FRAMES      : string := "";
		BUS_TRACE   : string := "";
		PIXELS      : string := "";
		CUTS        : string := "";
		BLOCKS      : string := "";
		-- BLOCKS from the power-on, not from the CPU's first access
		BLOCKS_FROM_POWER_ON : boolean := false;

		-- The chip: DMG_B, or the CGB's CGB_C or CGB_E (see gameboy)
		REVISION : revision_t := DMG_B;

		-- Without the sound channels, which are not simulated: faster (see
		-- dmg_cpu_b)
		WITH_CHANNELS : boolean := true;

		-- VRAM starting with zeros (see gameboy)
		VRAM_ZERO : boolean := false
	);
end entity;

architecture run of gameboy_tb is
	signal sin, sck : std_logic;
	signal sout     : std_ulogic;
	signal cpg, cp, cpl, fr, st, s : std_ulogic;
	signal ld       : std_ulogic_vector(1 downto 0);   -- the DMG's pixel
	signal rgb      : std_ulogic_vector(14 downto 0);  -- the CGB's
	signal so1, so2 : real;

	signal booted : boolean := false;

	-- The core's data bus in the middle of phase H, where a read takes it
	-- (for BLOCKS).
	signal read_in_h : std_logic_vector(7 downto 0) := (others => '0');

	-- The time, in ms, for the messages.
	impure function now_ms return string is
	begin
		return to_string(real(now / 1 ns) / 1.0e6, "%.1f") & " ms";
	end function;

	procedure write_file(path : string; contents : string) is
		file f    : text;
		variable l : line;
	begin
		if path /= "" then
			file_open(f, path, write_mode);
			write(l, contents);
			writeline(f, l);
			file_close(f);
		end if;
	end procedure;

	-- A byte in hexadecimal, as dmg-sim writes it.
	function hex(v : std_ulogic_vector(7 downto 0)) return string is
		constant digits : string(1 to 16) := "0123456789abcdef";
		variable n      : natural := to_integer(unsigned(to_01(v)));
	begin
		return digits(n / 16 + 1) & digits(n mod 16 + 1);
	end function;

	-- An M-cycle's number, from the time since the CPU's first access: whole
	-- M-cycles of the normal speed (952 ns, four periods of the 4 MiHz
	-- clock), and ".5" for an M-cycle that starts in the second half of one,
	-- which double speed has. Counting time, not the CPU's M-cycles, the
	-- numbers stay ceres's (whose bus_trace counts 8 MiHz units) through a
	-- speed switch and while the oscillator is stopped.
	function mcycle_at(since : delay_length) return string is
		constant half : delay_length := 476 ns;
		variable n    : natural := (since + half / 2) / half;
	begin
		if n mod 2 = 1 then
			return integer'image(n / 2) & ".5";
		end if;
		return integer'image(n / 2);
	end function;

	-- How far into phase H the traces look: in its middle, where the CPU's
	-- address and data have settled (they take up to 40 ns), in a phase of
	-- 119 ns, or of 59.5 ns in double speed.
	function in_phase_h(double_speed : std_ulogic) return delay_length is
	begin
		if double_speed = '1' then
			return 50 ns;
		end if;
		return 60 ns;
	end function;

	procedure end_with(result : string) is
	begin
		report "result: " & result & " (" & now_ms & ")";
		write_file(RESULT_FILE, result);
		finish;
	end procedure;
begin
	u_gameboy : entity work.gameboy
		generic map (
			ROM => ROM, BOOTROM => BOOTROM, REVISION => REVISION, WITH_CHANNELS => WITH_CHANNELS,
			VRAM_ZERO => VRAM_ZERO
		)
		port map (
			sin => sin, sout => sout, sck => sck,
			cpg => cpg, cp => cp, cpl => cpl, fr => fr, st => st, s => s, ld => ld, rgb => rgb,
			so1 => so1, so2 => so2
		);

	-- The run's time limit, counted from the end of the reset.
	time_limit : process
		alias cpu_sys_reset is << signal .gameboy_tb.u_gameboy.chip.u_chip.cpu_sys_reset : std_ulogic >>;
	begin
		wait until falling_edge(cpu_sys_reset);
		report "the reset is done (" & now_ms & "), " & to_string(SECS) & " s to run";
		wait for SECS * 1 sec;
		if TEST /= "" then
			end_with("timeout");
		else
			end_with("done");
		end if;
	end process;

	-- The CPU has read the entry point of the cartridge (0100): the boot ROM
	-- is done, and the state of the chip is meaningful to the tests.
	entry : process
		alias a    is << signal .gameboy_tb.u_gameboy.a : std_logic_vector(15 downto 0) >>;
		alias rd_n is << signal .gameboy_tb.u_gameboy.rd_n : std_logic >>;
	begin
		wait until to_x01(rd_n) = '0' and a = x"0100";
		booted <= true;
		report "the CPU reads the entry point 0100 (" & now_ms & ")";
		wait;
	end process;

	-- Mooneye: the test ends with LD B,B (opcode 40), with the Fibonacci
	-- numbers 3, 5, 8, 13, 21 and 34 in B, C, D, E, H and L if it passed.
	mooneye : process
		alias opcode is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.opcode : std_ulogic_vector(7 downto 0) >>;
		alias b is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.b : std_ulogic_vector(7 downto 0) >>;
		alias c is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.c_reg : std_ulogic_vector(7 downto 0) >>;
		alias d is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.d : std_ulogic_vector(7 downto 0) >>;
		alias e is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.e : std_ulogic_vector(7 downto 0) >>;
		alias h is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.h : std_ulogic_vector(7 downto 0) >>;
		alias l is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.l : std_ulogic_vector(7 downto 0) >>;
	begin
		if TEST /= "mooneye" then
			wait;
		end if;
		wait until booted and opcode = x"40";
		-- Let the instruction finish.
		wait for 2 us;
		if b & c & d & e & h & l = x"0305080D1522" then
			end_with("pass");
		else
			end_with("fail B=" & hex(b) & " C=" & hex(c) & " D=" & hex(d)
				& " E=" & hex(e) & " H=" & hex(h) & " L=" & hex(l));
		end if;
	end process;

	-- gbmicrotest: the test writes its result to FF80, the expected value
	-- to FF81, and 01 (passed) or FF (failed) to FF82. RAM is written as long
	-- as the CPU writes, and the bus holds the precharged FF before the CPU
	-- drives the byte: a result counts once it holds for 2 us.
	gbmicrotest : process
		alias ram is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_high_ram.ram : ram_t >>;
	begin
		if TEST /= "gbmicrotest" then
			wait;
		end if;
		loop
			wait for 1 us;
			if ram(2) = x"01" or ram(2) = x"FF" then
				wait for 2 us;
				if ram(2) = x"01" then
					end_with("pass FF80=" & hex(ram(0)) & " FF81=" & hex(ram(1)));
				elsif ram(2) = x"FF" then
					end_with("fail FF80=" & hex(ram(0)) & " FF81=" & hex(ram(1)));
				end if;
			end if;
		end loop;
	end process;

	-- The other end of the serial link: what the Game Boy sends with its
	-- internal clock, a byte with the most significant bit first, sampled on
	-- the rising edge of SCK. Blargg's tests print their result there.
	serial : process
		variable byte_in  : std_ulogic_vector(7 downto 0);
		variable received : line := new string'("");
		variable old      : line;
	begin
		loop
			for i in 1 to 8 loop
				wait until rising_edge(sck);
				byte_in := byte_in(6 downto 0) & to_x01(sout);
			end loop;
			old := received;
			received := new string'(old.all & character'val(to_integer(unsigned(to_01(byte_in)))));
			deallocate(old);
			write_file(SERIAL_FILE, received.all);
			if TEST = "blargg" and received'length >= 6 then
				if received(received'high - 5 to received'high) = "Passed" then
					end_with("pass serial: " & received.all);
				elsif received(received'high - 5 to received'high) = "Failed" then
					end_with("fail serial: " & received.all);
				end if;
			end if;
		end loop;
	end process;

	access_trace : if REVISION = DMG_B generate
		-- The CPU's accesses, seen in the middle of phase H: the M-cycle's
		-- address and the byte on the core's data bus, read or written. The
		-- phases run on through HALT, which stops the core's clocks (no
		-- data_phase then), so that M-cycles count time.
		accesses : process
			alias phase      is << signal .gameboy_tb.u_gameboy.chip.u_chip.phase : phase_t >>;
			alias data_phase is << signal .gameboy_tb.u_gameboy.chip.u_chip.data_phase : std_ulogic >>;
			alias c          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.c : controls_t >>;
			alias address    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.idu_in : std_ulogic_vector(15 downto 0) >>;
			alias db         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.db : std_logic_vector(7 downto 0) >>;
			file f            : text;
			variable l        : line;
			variable mcycle   : natural := 0;
			variable started  : boolean := false;
			variable kind     : character;
		begin
			if BUS_TRACE = "" then
				wait;
			end if;
			file_open(f, BUS_TRACE, write_mode);
			loop
				wait until phase = A or phase = H;
				if phase = A then
					mcycle := mcycle + 1;
				else
					wait for 60 ns;
					if data_phase = '1' and (c.fetch = '1' or c.mread = '1' or c.mwrite = '1') then
						if not started then
							started := true;
							mcycle  := 0;
						end if;
						kind := 'W' when c.mwrite = '1' else 'R';
						write(l, integer'image(mcycle) & ' ' & kind & ' ' & to_hstring(address) & ' '
							& to_hstring(to_x01(db)));
						writeline(f, l);
					end if;
				end if;
			end loop;
		end process;
	else generate
		-- The CPU's accesses, seen in the middle of phase H: the M-cycle's
		-- address and the byte on the core's data bus, read or written, numbered
		-- by mcycle_at. The phases are the core's, which stop while HDMA holds
		-- it.
		accesses : process
			alias double_speed is << signal .gameboy_tb.u_gameboy.chip.u_chip.double_speed : std_ulogic >>;
			alias phase      is << signal .gameboy_tb.u_gameboy.chip.u_chip.phase_core : phase_t >>;
			alias data_phase is << signal .gameboy_tb.u_gameboy.chip.u_chip.data_phase_core : std_ulogic >>;
			alias c          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.c : controls_t >>;
			alias address    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.idu_in : std_ulogic_vector(15 downto 0) >>;
			alias db         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.db : std_logic_vector(7 downto 0) >>;
			file f            : text;
			variable l        : line;
			variable started  : boolean := false;
			variable first_at : delay_length := 0 ns;
			variable kind     : character;
		begin
			if BUS_TRACE = "" then
				wait;
			end if;
			file_open(f, BUS_TRACE, write_mode);
			loop
				wait until phase = H;
				wait for in_phase_h(double_speed);
				if data_phase = '1' and (c.fetch = '1' or c.mread = '1' or c.mwrite = '1') then
					if not started then
						started  := true;
						first_at := now;
					end if;
					kind := 'W' when c.mwrite = '1' else 'R';
					write(l, mcycle_at(now - first_at) & ' ' & kind & ' ' & to_hstring(address) & ' '
						& to_hstring(to_x01(db)));
					writeline(f, l);
				end if;
			end loop;
		end process;
	end generate;

	-- The divider, the timer and IF at the end of every phase: sampled as the
	-- next phase starts, before the flip-flops change (tco after the edge).
	-- The byte a read takes: the core's data bus in the middle of phase H,
	-- as the accesses' trace sees it.
	read_taken : process
		alias phase is << signal .gameboy_tb.u_gameboy.chip.u_chip.phase : phase_t >>;
		alias db    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.db : std_logic_vector(7 downto 0) >>;
	begin
		if BLOCKS = "" then
			wait;
		end if;
		loop
			wait until phase = H;
			wait for 60 ns;
			read_in_h <= db;
		end loop;
	end process;

	blocks_traces : if WITH_CHANNELS generate
	blocks_trace : process
		alias phase       is << signal .gameboy_tb.u_gameboy.chip.u_chip.phase : phase_t >>;
		alias data_phase  is << signal .gameboy_tb.u_gameboy.chip.u_chip.data_phase : std_ulogic >>;
		alias c           is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.c : controls_t >>;
		alias addr        is << signal .gameboy_tb.u_gameboy.chip.u_chip.addr : std_logic_vector(15 downto 0) >>;
		alias data        is << signal .gameboy_tb.u_gameboy.chip.u_chip.data : std_logic_vector(7 downto 0) >>;
		alias cpu_rd      is << signal .gameboy_tb.u_gameboy.chip.u_chip.cpu_rd : std_ulogic >>;
		alias cpu_wr      is << signal .gameboy_tb.u_gameboy.chip.u_chip.cpu_wr : std_ulogic >>;
		alias div         is << signal .gameboy_tb.u_gameboy.chip.u_chip.div : unsigned(15 downto 0) >>;
		alias tima        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_timer.tima : std_ulogic_vector(7 downto 0) >>;
		alias tma         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_timer.tma : std_ulogic_vector(7 downto 0) >>;
		alias tac         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_timer.tac : std_ulogic_vector(2 downto 0) >>;
		alias int_timer   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_timer.int_timer_i : std_ulogic >>;
		alias bit7_before is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_timer.bit7_before : std_ulogic >>;
		alias timer_clock is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_timer.timer_clock : std_ulogic >>;
		alias write_tma   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_timer.write_tma : std_ulogic >>;
		alias write_tac   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_timer.write_tac : std_ulogic >>;
		alias int_vbl     is << signal .gameboy_tb.u_gameboy.chip.u_chip.int_vbl : std_ulogic >>;
		alias int_stat    is << signal .gameboy_tb.u_gameboy.chip.u_chip.int_stat : std_ulogic >>;
		alias int_serial  is << signal .gameboy_tb.u_gameboy.chip.u_chip.int_serial : std_ulogic >>;
		alias int_jp      is << signal .gameboy_tb.u_gameboy.chip.u_chip.int_jp : std_ulogic >>;
		alias acks        is << signal .gameboy_tb.u_gameboy.chip.u_chip.acks : std_ulogic_vector(4 downto 0) >>;
		alias flags       is << signal .gameboy_tb.u_gameboy.chip.u_chip.flags : std_ulogic_vector(4 downto 0) >>;
		alias read_value  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_interrupts.read_value : std_ulogic_vector(4 downto 0) >>;
		alias ie          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_interrupts.ie : std_ulogic_vector(7 downto 0) >>;
		alias dma_source  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_dma.source : std_ulogic_vector(7 downto 0) >>;
		alias dma_req     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_dma.requested : std_ulogic >>;
		alias dma_start_1 is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_dma.start_1 : std_ulogic >>;
		alias dma_start_2 is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_dma.start_2 : std_ulogic >>;
		alias dma_enabled is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_dma.enabled : std_ulogic >>;
		alias dma_done    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_dma.done : std_ulogic >>;
		alias dma_run     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_dma.run : std_ulogic >>;
		alias dma_count   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_dma.count : unsigned(7 downto 0) >>;
		alias dma_write   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam.dma_write : std_ulogic >>;
		alias dma_phi     is << signal .gameboy_tb.u_gameboy.chip.u_chip.dma_phi : std_ulogic >>;
		alias clk         is << signal .gameboy_tb.u_gameboy.chip.u_chip.clk : std_ulogic >>;
		alias ppu_reset   is << signal .gameboy_tb.u_gameboy.chip.u_chip.ppu_reset : std_ulogic >>;
		alias mode0       is << signal .gameboy_tb.u_gameboy.chip.u_chip.mode0 : std_ulogic >>;
		alias mode2       is << signal .gameboy_tb.u_gameboy.chip.u_chip.mode2 : std_ulogic >>;
		alias mode3       is << signal .gameboy_tb.u_gameboy.chip.u_chip.mode3 : std_ulogic >>;
		alias wuvu        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.wuvu_i : std_ulogic >>;
		alias vena        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.vena : std_ulogic >>;
		alias wosu        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.wosu_i : std_ulogic >>;
		alias lx          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.lx_i : unsigned(6 downto 0) >>;
		alias vclk        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.vclk_i : std_ulogic >>;
		alias nype        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.nype : std_ulogic >>;
		alias ly          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.ly_i : unsigned(7 downto 0) >>;
		alias line_153    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.line_153 : std_ulogic >>;
		alias first_line  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.first_line : std_ulogic >>;
		alias mode1       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.mode1_i : std_ulogic >>;
		alias lyc         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.lyc : std_ulogic_vector(7 downto 0) >>;
		alias lyc_match   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.lyc_match : std_ulogic >>;
		alias stat_enable is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.stat_enable : std_ulogic_vector(6 downto 3) >>;
		alias lcdc        is << signal .gameboy_tb.u_gameboy.chip.u_chip.lcdc : std_ulogic_vector(7 downto 0) >>;
		alias sprite_y    is << signal .gameboy_tb.u_gameboy.chip.u_chip.oam_even : std_ulogic_vector(7 downto 0) >>;
		alias save        is << signal .gameboy_tb.u_gameboy.chip.u_chip.save : std_ulogic_vector(9 downto 0) >>;
		alias y_match     is << signal .gameboy_tb.u_gameboy.chip.u_chip.y_match : std_ulogic >>;
		alias start_scan  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam_scan.start_scan : std_ulogic >>;
		alias anel        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam_scan.anel : std_ulogic >>;
		alias scanning    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam_scan.scanning_i : std_ulogic >>;
		alias ceno        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam_scan.ceno_i : std_ulogic >>;
		alias byba        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam_scan.byba : std_ulogic >>;
		alias doba        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam_scan.doba : std_ulogic >>;
		alias not_saving  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam_scan.not_saving : std_ulogic >>;
		alias scan_index  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam_scan.index_i : unsigned(5 downto 0) >>;
		alias scan_count  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam_scan.count : unsigned(3 downto 0) >>;
		alias first_even  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam.first_even : std_ulogic_vector(7 downto 0) >>;
		alias fetch_read  is << signal .gameboy_tb.u_gameboy.chip.u_chip.fetch_read : std_ulogic >>;
		alias fetch_hold  is << signal .gameboy_tb.u_gameboy.chip.u_chip.fetch_hold : std_ulogic >>;
		alias ck_ck1        is << signal .gameboy_tb.u_gameboy.chip.u_chip.ck1 : std_ulogic >>;
		alias ck_reset_n    is << signal .gameboy_tb.u_gameboy.chip.u_chip.reset_n : std_ulogic >>;
		alias ck_t1         is << signal .gameboy_tb.u_gameboy.chip.u_chip.t1 : std_ulogic >>;
		alias ck_t2         is << signal .gameboy_tb.u_gameboy.chip.u_chip.t2 : std_ulogic >>;
		alias ck_osc_ena    is << signal .gameboy_tb.u_gameboy.chip.u_chip.osc_ena : std_ulogic >>;
		alias ck_clk_ena    is << signal .gameboy_tb.u_gameboy.chip.u_chip.clk_ena : std_ulogic >>;
		alias ck_mreq       is << signal .gameboy_tb.u_gameboy.chip.u_chip.mreq : std_ulogic >>;
		alias ck_write      is << signal .gameboy_tb.u_gameboy.chip.u_chip.write : std_ulogic >>;
		alias ck_test_reset is << signal .gameboy_tb.u_gameboy.chip.u_chip.test_reset : std_ulogic >>;
		alias ck_afur       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_clocks.afur : std_ulogic >>;
		alias ck_alef       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_clocks.alef : std_ulogic >>;
		alias ck_apuk       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_clocks.apuk : std_ulogic >>;
		alias ck_adyk       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_clocks.adyk : std_ulogic >>;
		alias ck_waiting    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_clocks.waiting : std_ulogic >>;
		alias ck_osc_stable is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_clocks.osc_stable_i : std_ulogic >>;
		alias ck_sys_reset  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_clocks.sys_reset_i : std_ulogic >>;
		alias ck_cpu_reset  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_clocks.cpu_sys_reset_i : std_ulogic >>;
		alias ck_reset2     is << signal .gameboy_tb.u_gameboy.chip.u_chip.reset2 : std_ulogic >>;
		alias ck_1mhz       is << signal .gameboy_tb.u_gameboy.chip.u_chip.clk_1mhz : std_ulogic >>;
		alias ck_t4         is << signal .gameboy_tb.u_gameboy.chip.u_chip.clk_t4 : std_ulogic >>;
		alias ck_data       is << signal .gameboy_tb.u_gameboy.chip.u_chip.data_phase : std_ulogic >>;
		alias ck_exec       is << signal .gameboy_tb.u_gameboy.chip.u_chip.exec_phase : std_ulogic >>;
		alias ck_pch        is << signal .gameboy_tb.u_gameboy.chip.u_chip.pch_phase : std_ulogic >>;
		alias ck_wr_sync    is << signal .gameboy_tb.u_gameboy.chip.u_chip.cpu_wr_sync : std_ulogic >>;
		alias ck_ext_cs_en  is << signal .gameboy_tb.u_gameboy.chip.u_chip.cpu_ext_cs_en : std_ulogic >>;
		alias ck_phi        is << signal .gameboy_tb.u_gameboy.chip.u_chip.phi : std_ulogic >>;
		alias r_mode3_i     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.mode3_i : std_ulogic >>;
		alias r_voga        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.voga : std_ulogic >>;
		alias r_px9         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.px9 : std_ulogic >>;
		alias r_lyzu        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.lyzu : std_ulogic >>;
		alias r_lovy        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.lovy : std_ulogic >>;
		alias r_fetching_i  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.fetching_i : std_ulogic >>;
		alias r_nyka        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.nyka : std_ulogic >>;
		alias r_pory        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.pory : std_ulogic >>;
		alias r_pygo        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.pygo : std_ulogic >>;
		alias r_started     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.started : std_ulogic >>;
		alias r_sary        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.sary : std_ulogic >>;
		alias r_wy_latch    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.wy_latch : std_ulogic >>;
		alias r_pyco        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.pyco : std_ulogic >>;
		alias r_nunu        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.nunu : std_ulogic >>;
		alias r_nopa        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.nopa : std_ulogic >>;
		alias r_in_window   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.in_window : std_ulogic >>;
		alias r_rydy        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.rydy : std_ulogic >>;
		alias r_sovy        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.sovy : std_ulogic >>;
		alias r_puxa        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.puxa : std_ulogic >>;
		alias r_nyze        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.nyze : std_ulogic >>;
		alias r_paho        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.paho : std_ulogic >>;
		alias r_discard     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.discard : std_ulogic >>;
		alias r_ryfa        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.ryfa : std_ulogic >>;
		alias r_rene        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.rene : std_ulogic >>;
		alias r_line_started is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.line_started : std_ulogic >>;
		alias r_sobu        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.sobu : std_ulogic >>;
		alias r_suda        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.suda : std_ulogic >>;
		alias r_busy        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.busy : std_ulogic >>;
		alias r_tobu        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.tobu : std_ulogic >>;
		alias r_vonu        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.vonu : std_ulogic >>;
		alias r_seba        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.seba : std_ulogic >>;
		alias r_tyfo        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.tyfo : std_ulogic >>;
		alias r_px          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.px_i : unsigned(7 downto 0) >>;
		alias r_fetch_count is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.fetch_count : unsigned(2 downto 0) >>;
		alias r_fine        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.fine : unsigned(2 downto 0) >>;
		alias r_sprite_step is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.sprite_step : unsigned(2 downto 0) >>;
		alias r_win_x       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.win_x : unsigned(4 downto 0) >>;
		alias r_win_y       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.win_y : unsigned(7 downto 0) >>;
		alias r_bg_tile     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.bg_tile : std_ulogic_vector(7 downto 0) >>;
		alias r_bg_low      is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.bg_low : std_ulogic_vector(7 downto 0) >>;
		alias r_bg_shift_a  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.bg_shift_a : std_ulogic_vector(7 downto 0) >>;
		alias r_bg_shift_b  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.bg_shift_b : std_ulogic_vector(7 downto 0) >>;
		alias r_sp_low_a    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.sp_low_a : std_ulogic_vector(7 downto 0) >>;
		alias r_sp_low_b    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.sp_low_b : std_ulogic_vector(7 downto 0) >>;
		alias r_sp_a        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.sp_a : std_ulogic_vector(7 downto 0) >>;
		alias r_sp_b        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.sp_b : std_ulogic_vector(7 downto 0) >>;
		alias r_sp_palette  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.sp_palette : std_ulogic_vector(7 downto 0) >>;
		alias r_sp_priority is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.sp_priority : std_ulogic_vector(7 downto 0) >>;
		alias r_scy         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.scy : std_ulogic_vector(7 downto 0) >>;
		alias r_scx         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.scx : std_ulogic_vector(7 downto 0) >>;
		alias r_wy          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.wy : std_ulogic_vector(7 downto 0) >>;
		alias r_wx          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.wx : std_ulogic_vector(7 downto 0) >>;
		alias r_bgp         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.bgp : std_ulogic_vector(7 downto 0) >>;
		alias r_obp0        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.obp0 : std_ulogic_vector(7 downto 0) >>;
		alias r_obp1        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.obp1 : std_ulogic_vector(7 downto 0) >>;
		alias r_line_start  is << signal .gameboy_tb.u_gameboy.chip.u_chip.line_start : std_ulogic >>;
		alias r_stop_scan   is << signal .gameboy_tb.u_gameboy.chip.u_chip.stop_scan : std_ulogic >>;
		alias r_x_match     is << signal .gameboy_tb.u_gameboy.chip.u_chip.x_match : std_ulogic >>;
		alias r_sprite_loaded is << signal .gameboy_tb.u_gameboy.chip.u_chip.sprite_loaded : std_ulogic >>;
		alias r_rendering   is << signal .gameboy_tb.u_gameboy.chip.u_chip.rendering : std_ulogic >>;
		alias r_md          is << signal .gameboy_tb.u_gameboy.chip.u_chip.vram_data : std_logic_vector(7 downto 0) >>;
		alias r_odd         is << signal .gameboy_tb.u_gameboy.chip.u_chip.oam_odd : std_ulogic_vector(7 downto 0) >>;
		alias r_line        is << signal .gameboy_tb.u_gameboy.chip.u_chip.render_line : std_logic_vector(3 downto 0) >>;
		alias r_ma          is << signal .gameboy_tb.u_gameboy.chip.u_chip.render_ma : std_logic_vector(12 downto 0) >>;
		alias r_cp          is << signal .gameboy_tb.u_gameboy.chip.u_chip.cp : std_ulogic >>;
		alias r_st          is << signal .gameboy_tb.u_gameboy.chip.u_chip.st : std_ulogic >>;
		alias r_ld          is << signal .gameboy_tb.u_gameboy.chip.u_chip.ld : std_ulogic_vector(1 downto 0) >>;
		alias s_scanned     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_sprite_store.scanned : std_ulogic_vector(5 downto 0) >>;
		alias s_fetched     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_sprite_store.fetched : std_ulogic_vector(9 downto 0) >>;
		alias s_xs          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_sprite_store.xs : x_t >>;
		alias s_lines       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_sprite_store.lines : line_t >>;
		alias s_indexes     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_sprite_store.indexes : index_t >>;
		alias x_d           is << signal .gameboy_tb.u_gameboy.chip.u_chip.d : std_logic_vector(7 downto 0) >>;
		alias x_non_vram    is << signal .gameboy_tb.u_gameboy.chip.u_chip.non_vram_mreq : std_ulogic >>;
		alias x_dma_ext     is << signal .gameboy_tb.u_gameboy.chip.u_chip.dma_addr_ext : std_ulogic >>;
		alias x_vram_oam    is << signal .gameboy_tb.u_gameboy.chip.u_chip.vram_to_oam : std_ulogic >>;
		alias x_wuje        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam.wuje : std_ulogic >>;
		alias x_maka        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam.maka : std_ulogic >>;
		alias x_first_odd   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam.first_odd : std_ulogic_vector(7 downto 0) >>;
		alias x_bus_even    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam.bus_even : std_ulogic_vector(7 downto 0) >>;
		alias x_bus_odd     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam.bus_odd : std_ulogic_vector(7 downto 0) >>;
		alias x_vram_addr   is << signal .gameboy_tb.u_gameboy.chip.u_chip.vram_addr : std_logic_vector(13 downto 0) >>;
		alias x_mcs_n       is << signal .gameboy_tb.u_gameboy.chip.u_chip.mcs_n : std_logic >>;
		alias x_mrd_n       is << signal .gameboy_tb.u_gameboy.chip.u_chip.mrd_n : std_logic >>;
		alias x_mwr_n       is << signal .gameboy_tb.u_gameboy.chip.u_chip.mwr_n : std_logic >>;
		alias y_mreq        is << signal .gameboy_tb.u_gameboy.chip.u_chip.mreq : std_ulogic >>;
		alias y_read        is << signal .gameboy_tb.u_gameboy.chip.u_chip.read : std_ulogic >>;
		alias y_write       is << signal .gameboy_tb.u_gameboy.chip.u_chip.write : std_ulogic >>;
		alias y_a           is << signal .gameboy_tb.u_gameboy.chip.u_chip.a : std_logic_vector(15 downto 0) >>;
		alias y_rd_n        is << signal .gameboy_tb.u_gameboy.chip.u_chip.rd_n : std_logic >>;
		alias y_wr_n        is << signal .gameboy_tb.u_gameboy.chip.u_chip.wr_n : std_logic >>;
		alias y_cs_n        is << signal .gameboy_tb.u_gameboy.chip.u_chip.cs_n : std_ulogic >>;
		alias y_lat_addr    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_external_bus.latched_addr : std_ulogic_vector(14 downto 0) >>;
		alias y_lat_data    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_external_bus.latched_data : std_ulogic_vector(7 downto 0) >>;
		alias y_unmapped    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_boot_rom.unmapped : std_ulogic >>;
		alias y_boot_sel    is << signal .gameboy_tb.u_gameboy.chip.u_chip.boot_sel : std_ulogic >>;
		alias y_p1          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_joypad.p1 : std_ulogic_vector(7 downto 0) >>;
		alias y_jp_read     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_joypad.read_value : std_ulogic_vector(3 downto 0) >>;
		alias y_jp_hist     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_joypad.history : std_ulogic_vector(3 downto 0) >>;
		alias y_jp_wake     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_joypad.wakeup : std_ulogic >>;
		alias y_p10_13      is << signal .gameboy_tb.u_gameboy.chip.u_chip.p10_13 : std_logic_vector(3 downto 0) >>;
		alias y_sb          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_serial.sb : std_ulogic_vector(7 downto 0) >>;
		alias y_transfer    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_serial.transfer : std_ulogic >>;
		alias y_int_clock   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_serial.internal_clock : std_ulogic >>;
		alias y_half        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_serial.half : std_ulogic >>;
		alias y_ser_count   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_serial.count : unsigned(3 downto 0) >>;
		alias y_ser_out     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_serial.ser_out : std_ulogic >>;
		alias y_sck         is << signal .gameboy_tb.u_gameboy.chip.u_chip.sck : std_logic >>;
		alias y_addr_latch  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_external_bus.addr_latch : std_ulogic >>;
		alias y_sin         is << signal .gameboy_tb.u_gameboy.chip.u_chip.sin : std_logic >>;
		alias q_halt            is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.halt_i : std_ulogic >>;
		alias q_stop            is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.stop_i : std_ulogic >>;
		alias q_halt_delayed    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.halt_delayed : std_ulogic >>;
		alias q_stop_delayed    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.stop_delayed : std_ulogic >>;
		alias q_pending_delayed is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.pending_delayed : std_ulogic >>;
		alias q_not_ready_1     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.not_ready_1 : std_ulogic >>;
		alias q_not_ready_2     is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.not_ready_2 : std_ulogic >>;
		alias q_ime_state       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.ime_state : std_ulogic >>;
		alias q_ime             is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.ime_i : std_ulogic >>;
		alias q_no_nmi          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.no_nmi : std_ulogic >>;
		alias q_no_nmi_1        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.no_nmi_1 : std_ulogic >>;
		alias q_entering        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.entering : std_ulogic >>;
		alias q_int_entry       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.int_entry_i : std_ulogic >>;
		alias q_sel_reg_w       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.sel_reg_w_i : std_ulogic >>;
		alias q_cb_next         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.cb_next : std_ulogic >>;
		alias q_cb_flag         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.cb_flag : std_ulogic >>;
		alias q_cond_fail       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_alu.cond_fail_i : std_ulogic >>;
		alias q_z7_master       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_alu.z7_master : std_ulogic >>;
		alias q_z7_delayed      is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_alu.z7_delayed : std_ulogic >>;
		alias q_opcode          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.opcode_i : std_ulogic_vector(7 downto 0) >>;
		alias q_ir_next         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.ir_next : std_ulogic_vector(7 downto 0) >>;
		alias q_mcyc            is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_sequencer.mcyc_i : std_ulogic_vector(2 downto 0) >>;
		alias q_flag_z          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_alu.flag_z : std_ulogic >>;
		alias q_flag_n          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_alu.flag_n : std_ulogic >>;
		alias q_flag_h          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_alu.flag_h : std_ulogic >>;
		alias q_flag_c          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_alu.flag_c : std_ulogic >>;
		alias q_a               is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.a : std_ulogic_vector(7 downto 0) >>;
		alias q_b               is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.b : std_ulogic_vector(7 downto 0) >>;
		alias q_c               is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.c_reg : std_ulogic_vector(7 downto 0) >>;
		alias q_d               is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.d : std_ulogic_vector(7 downto 0) >>;
		alias q_e               is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.e : std_ulogic_vector(7 downto 0) >>;
		alias q_h               is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.h : std_ulogic_vector(7 downto 0) >>;
		alias q_l               is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.l : std_ulogic_vector(7 downto 0) >>;
		alias q_w               is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.w : std_ulogic_vector(7 downto 0) >>;
		alias q_z               is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.z : std_ulogic_vector(7 downto 0) >>;
		alias q_sph             is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.sph : std_ulogic_vector(7 downto 0) >>;
		alias q_spl             is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.spl : std_ulogic_vector(7 downto 0) >>;
		alias q_pch             is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.pch : std_ulogic_vector(7 downto 0) >>;
		alias q_pcl             is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_registers.pcl : std_ulogic_vector(7 downto 0) >>;
		alias q_pending         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.u_interrupts.pending : std_ulogic_vector(7 downto 0) >>;
		alias q_db              is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.db : std_logic_vector(7 downto 0) >>;
		alias a_power        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.power : std_ulogic >>;
		alias a_test_i       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.test_i : std_ulogic >>;
		alias a_test_bit5    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.test_bit5 : std_ulogic >>;
		alias a_baza         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.baza : std_ulogic >>;
		alias a_ajer_i       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.ajer_i : std_ulogic >>;
		alias a_calo         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.calo : std_ulogic >>;
		alias a_atep         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.atep : std_ulogic >>;
		alias a_cemo         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.cemo : std_ulogic >>;
		alias a_cery         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.cery : std_ulogic >>;
		alias a_atyk         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.atyk : std_ulogic >>;
		alias a_avok         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.avok : std_ulogic >>;
		alias a_jeso         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.jeso : std_ulogic >>;
		alias a_bara         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.bara : std_ulogic >>;
		alias a_caru         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.caru : std_ulogic >>;
		alias a_bylu         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.bylu : std_ulogic >>;
		alias a_nr50         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.nr50 : std_ulogic_vector(7 downto 0) >>;
		alias a_nr51         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_apu_control.nr51 : std_ulogic_vector(7 downto 0) >>;
		alias c1_length_on   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.length_on : std_ulogic >>;
		alias c1_trigger     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.trigger : std_ulogic >>;
		alias c1_start       is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.start : std_ulogic >>;
		alias c1_triggered   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.triggered : std_ulogic >>;
		alias c1_restart     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.restart : std_ulogic >>;
		alias c1_restarted   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.restarted : std_ulogic >>;
		alias c1_running     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.running : std_ulogic >>;
		alias c1_frozen      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.frozen : std_ulogic >>;
		alias c1_active_i    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.active_i : std_ulogic >>;
		alias c1_reload      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.reload : std_ulogic >>;
		alias c1_wave        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.wave : std_ulogic >>;
		alias c1_expired     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.expired : std_ulogic >>;
		alias c1_hz64        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.hz64 : std_ulogic >>;
		alias c1_tick        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.tick : std_ulogic >>;
		alias c1_at_limit    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.at_limit : std_ulogic >>;
		alias c1_env_stopped is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.env_stopped : std_ulogic >>;
		alias c1_sweep_step  is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.sweep_step : std_ulogic >>;
		alias c1_shifting    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.shifting : std_ulogic >>;
		alias c1_shift_done  is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.shift_done : std_ulogic >>;
		alias c1_sweep       is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.sweep : std_ulogic_vector(6 downto 0) >>;
		alias c1_duty        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.duty : std_ulogic_vector(1 downto 0) >>;
		alias c1_envelope    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.envelope : std_ulogic_vector(7 downto 0) >>;
		alias c1_acc         is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.acc : std_ulogic_vector(10 downto 0) >>;
		alias c1_counter     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.counter : unsigned(10 downto 0) >>;
		alias c1_step        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.step : unsigned(2 downto 0) >>;
		alias c1_length      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.length : unsigned(5 downto 0) >>;
		alias c1_period      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.period : unsigned(2 downto 0) >>;
		alias c1_volume      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.volume : unsigned(3 downto 0) >>;
		alias c1_sweep_count is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.sweep_count : unsigned(2 downto 0) >>;
		alias c1_shifts      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.shifts : unsigned(2 downto 0) >>;
		alias c1_shifted     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.shifted : std_ulogic_vector(10 downto 0) >>;
		alias c1_augend      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.augend : std_ulogic_vector(10 downto 0) >>;
		alias c1_addend      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch1.addend : std_ulogic_vector(10 downto 0) >>;
		alias c2_length_on   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.length_on : std_ulogic >>;
		alias c2_trigger     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.trigger : std_ulogic >>;
		alias c2_start       is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.start : std_ulogic >>;
		alias c2_triggered   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.triggered : std_ulogic >>;
		alias c2_restart     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.restart : std_ulogic >>;
		alias c2_restarted   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.restarted : std_ulogic >>;
		alias c2_running     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.running : std_ulogic >>;
		alias c2_frozen      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.frozen : std_ulogic >>;
		alias c2_active_i    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.active_i : std_ulogic >>;
		alias c2_reload      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.reload : std_ulogic >>;
		alias c2_wave        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.wave : std_ulogic >>;
		alias c2_expired     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.expired : std_ulogic >>;
		alias c2_hz64        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.hz64 : std_ulogic >>;
		alias c2_tick        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.tick : std_ulogic >>;
		alias c2_at_limit    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.at_limit : std_ulogic >>;
		alias c2_env_stopped is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.env_stopped : std_ulogic >>;
		alias c2_duty        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.duty : std_ulogic_vector(1 downto 0) >>;
		alias c2_envelope    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.envelope : std_ulogic_vector(7 downto 0) >>;
		alias c2_frequency   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.frequency : std_ulogic_vector(10 downto 0) >>;
		alias c2_counter     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.counter : unsigned(10 downto 0) >>;
		alias c2_step        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.step : unsigned(2 downto 0) >>;
		alias c2_length      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.length : unsigned(5 downto 0) >>;
		alias c2_period      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.period : unsigned(2 downto 0) >>;
		alias c2_volume      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch2.volume : unsigned(3 downto 0) >>;
		alias c3_dac_on_i    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.dac_on_i : std_ulogic >>;
		alias c3_length_on   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.length_on : std_ulogic >>;
		alias c3_trigger     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.trigger : std_ulogic >>;
		alias c3_start       is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.start : std_ulogic >>;
		alias c3_triggered   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.triggered : std_ulogic >>;
		alias c3_restart     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.restart : std_ulogic >>;
		alias c3_restarted   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.restarted : std_ulogic >>;
		alias c3_running     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.running : std_ulogic >>;
		alias c3_frozen      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.frozen : std_ulogic >>;
		alias c3_playing     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.playing : std_ulogic >>;
		alias c3_active_i    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.active_i : std_ulogic >>;
		alias c3_reload      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.reload : std_ulogic >>;
		alias c3_wrapped     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.wrapped : std_ulogic >>;
		alias c3_expired     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.expired : std_ulogic >>;
		alias c3_busa        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.busa : std_ulogic >>;
		alias c3_bano        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.bano : std_ulogic >>;
		alias c3_fetch       is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.fetch : std_ulogic >>;
		alias c3_azet        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.azet : std_ulogic >>;
		alias c3_level       is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.level : std_ulogic_vector(1 downto 0) >>;
		alias c3_frequency   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.frequency : std_ulogic_vector(10 downto 0) >>;
		alias c3_counter     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.counter : unsigned(10 downto 0) >>;
		alias c3_position    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.position : unsigned(4 downto 0) >>;
		alias c3_length      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.length : unsigned(7 downto 0) >>;
		alias c3_bit_lines   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.bit_lines : std_ulogic_vector(7 downto 0) >>;
		alias c3_sample_byte is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.sample_byte : std_ulogic_vector(7 downto 0) >>;
		alias c3_ram         is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch3.ram : wave_ram_t >>;
		alias c4_length_on   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.length_on : std_ulogic >>;
		alias c4_trigger     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.trigger : std_ulogic >>;
		alias c4_start       is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.start : std_ulogic >>;
		alias c4_triggered   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.triggered : std_ulogic >>;
		alias c4_restart     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.restart : std_ulogic >>;
		alias c4_restarted   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.restarted : std_ulogic >>;
		alias c4_running     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.running : std_ulogic >>;
		alias c4_frozen      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.frozen : std_ulogic >>;
		alias c4_active_i    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.active_i : std_ulogic >>;
		alias c4_divided     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.divided : std_ulogic >>;
		alias c4_joto        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.joto : std_ulogic >>;
		alias c4_expired     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.expired : std_ulogic >>;
		alias c4_hz64        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.hz64 : std_ulogic >>;
		alias c4_tick        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.tick : std_ulogic >>;
		alias c4_at_limit    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.at_limit : std_ulogic >>;
		alias c4_env_stopped is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.env_stopped : std_ulogic >>;
		alias c4_envelope    is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.envelope : std_ulogic_vector(7 downto 0) >>;
		alias c4_noise       is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.noise : std_ulogic_vector(7 downto 0) >>;
		alias c4_divider     is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.divider : unsigned(2 downto 0) >>;
		alias c4_prescaler   is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.prescaler : unsigned(13 downto 0) >>;
		alias c4_lfsr        is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.lfsr : std_ulogic_vector(15 downto 1) >>;
		alias c4_length      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.length : unsigned(5 downto 0) >>;
		alias c4_period      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.period : unsigned(2 downto 0) >>;
		alias c4_volume      is << signal .gameboy_tb.u_gameboy.chip.u_chip.channels.u_apu_ch4.volume : unsigned(3 downto 0) >>;
		file f        : text;
		variable l    : line;
		variable seen : boolean := BLOCKS_FROM_POWER_ON;
		variable ended : phase_t;
		-- The requests that rose in the phase, also for less than its length.
		variable rose : std_ulogic_vector(4 downto 0) := (others => '0');
		-- The external bus's address latches were open in the phase, also
		-- for less than its length (the core's mreq pulses as an access
		-- inside the chip starts).
		variable latch_seen : std_ulogic := '0';
		-- Mode 0 rose in the phase, also for less than its length (a sprite
		-- at X 167 stops mode 3 again 4 ns after it ends).
		variable mode0_rose : std_ulogic := '0';
		-- The chip's data bus's bits that were a strong 0 in the phase.
		variable data_low : std_ulogic_vector(7 downto 0) := (others => '0');

		function b01(b : std_ulogic) return character is
		begin
			if to_x01(b) = '1' then
				return '1';
			end if;
			return '0';
		end function;

		function hx(v : std_ulogic_vector) return string is
		begin
			return to_hstring(to_01(v));
		end function;
	begin
		if BLOCKS = "" then
			wait;
		end if;
		file_open(f, BLOCKS, write_mode);
		ended := phase;
		loop
			wait on phase, int_vbl, int_stat, int_timer, int_serial, int_jp, y_addr_latch, data, mode0;
			if y_addr_latch = '1' then latch_seen := '1'; end if;
			if rising_edge(mode0) then mode0_rose := '1'; end if;
			for i in data'range loop
				if data(i) = '0' then data_low(i) := '1'; end if;
			end loop;
			if rising_edge(int_vbl) then rose(0) := '1'; end if;
			if rising_edge(int_stat) then rose(1) := '1'; end if;
			if rising_edge(int_timer) then rose(2) := '1'; end if;
			if rising_edge(int_serial) then rose(3) := '1'; end if;
			if rising_edge(int_jp) then rose(4) := '1'; end if;
			next when not phase'event;
			if not seen and data_phase = '1' and (c.fetch = '1' or c.mread = '1' or c.mwrite = '1') then
				seen := true;
			end if;
			if seen then
				write(l, phase_t'image(ended) & ' ' & hx(addr) & ' ' & b01(cpu_rd) & b01(cpu_wr) & ' ' & hx(data)
					& ' ' & hx(std_logic_vector(div)) & ' ' & hx(tima) & ' ' & hx(tma) & ' ' & hx(tac)
					& ' ' & b01(int_timer) & b01(bit7_before) & b01(timer_clock) & b01(write_tma) & b01(write_tac)
					& ' ' & hx(int_jp & int_serial & int_timer & int_stat & int_vbl) & ' ' & hx(acks)
					& ' ' & hx(flags) & ' ' & hx(read_value) & ' ' & hx(ie) & ' ' & hx(rose)
					& ' ' & hx(dma_source) & ' ' & b01(dma_req) & b01(dma_start_1) & b01(dma_start_2)
					& b01(dma_enabled) & b01(dma_done) & b01(dma_run) & ' ' & hx(std_ulogic_vector(dma_count))
					& ' ' & b01(dma_write) & ' ' & b01(dma_phi)
					& ' ' & b01(clk) & b01(ppu_reset) & b01(mode0) & b01(mode2) & b01(mode3)
					& b01(mode0_rose and not mode0)
					& ' ' & b01(wuvu) & b01(vena) & b01(wosu) & b01(vclk) & b01(nype) & b01(line_153)
					& b01(first_line) & b01(mode1) & b01(lyc_match)
					& ' ' & hx(std_ulogic_vector(lx)) & ' ' & hx(std_ulogic_vector(ly)) & ' ' & hx(lyc)
					& ' ' & hx(stat_enable & "000") & ' ' & hx(read_in_h)
					& ' ' & b01(start_scan) & b01(anel) & b01(scanning) & b01(ceno) & b01(byba) & b01(doba)
					& b01(not_saving) & b01(y_match) & b01(lcdc(2))
					& ' ' & hx(std_ulogic_vector(scan_index)) & ' ' & hx(std_ulogic_vector(scan_count))
					& ' ' & hx(save) & ' ' & hx(sprite_y)
					& ' ' & hx(first_even) & ' ' & b01(fetch_read) & b01(fetch_hold)
					& ' ' & b01(ck_ck1) & b01(ck_reset_n) & b01(ck_t1) & b01(ck_t2) & b01(ck_osc_ena)
					& b01(ck_clk_ena) & b01(ck_mreq) & b01(ck_write) & b01(ck_test_reset) & b01(div(15))
					& ' ' & b01(ck_afur) & b01(ck_alef) & b01(ck_apuk) & b01(ck_adyk) & b01(ck_waiting)
					& b01(ck_osc_stable) & b01(ck_sys_reset) & b01(ck_cpu_reset) & b01(ck_reset2)
					& ' ' & b01(ck_1mhz) & b01(ck_t4) & b01(ck_data) & b01(ck_exec) & b01(ck_pch)
					& b01(ck_wr_sync) & b01(ck_ext_cs_en) & b01(ck_phi));
				-- Render and the sprite store.
				write(l, ' ' & b01(r_mode3_i) & b01(r_voga) & b01(r_px9) & b01(r_lyzu) & b01(r_lovy) & b01(r_fetching_i) & b01(r_nyka) & b01(r_pory) & b01(r_pygo) & b01(r_started) & b01(r_sary) & b01(r_wy_latch) & b01(r_pyco) & b01(r_nunu) & b01(r_nopa) & b01(r_in_window) & b01(r_rydy) & b01(r_sovy) & b01(r_puxa) & b01(r_nyze) & b01(r_paho) & b01(r_discard) & b01(r_ryfa) & b01(r_rene) & b01(r_line_started) & b01(r_sobu) & b01(r_suda) & b01(r_busy) & b01(r_tobu) & b01(r_vonu) & b01(r_seba) & b01(r_tyfo)
					& ' ' & hx(std_ulogic_vector(r_px)) & ' ' & hx(std_ulogic_vector(r_fetch_count))
					& ' ' & hx(std_ulogic_vector(r_fine)) & ' ' & hx(std_ulogic_vector(r_sprite_step))
					& ' ' & hx(std_ulogic_vector(r_win_x)) & ' ' & hx(std_ulogic_vector(r_win_y))
					& ' ' & hx(r_bg_tile) & ' ' & hx(r_bg_low) & ' ' & hx(r_bg_shift_a) & ' ' & hx(r_bg_shift_b)
					& ' ' & hx(r_sp_low_a) & ' ' & hx(r_sp_low_b) & ' ' & hx(r_sp_a) & ' ' & hx(r_sp_b)
					& ' ' & hx(r_sp_palette) & ' ' & hx(r_sp_priority)
					& ' ' & hx(r_scy) & ' ' & hx(r_scx) & ' ' & hx(r_wy) & ' ' & hx(r_wx)
					& ' ' & hx(r_bgp) & ' ' & hx(r_obp0) & ' ' & hx(r_obp1)
					& ' ' & b01(r_line_start) & b01(r_stop_scan) & b01(r_x_match) & b01(r_sprite_loaded) & b01(r_rendering)
					& ' ' & hx(r_md) & ' ' & hx(r_odd) & ' ' & hx(r_line)
					& ' ' & hx(r_ma) & ' ' & b01(r_cp) & b01(r_st) & ' ' & hx(r_ld)
					& ' ' & hx(s_scanned) & ' ' & hx(s_fetched) & ' ');
				for i in 0 to 9 loop
					write(l, hx(s_xs(i)));
				end loop;
				write(l, ' ');
				for i in 0 to 9 loop
					write(l, hx(s_lines(i)));
				end loop;
				write(l, ' ');
				for i in 0 to 9 loop
					write(l, hx(s_indexes(i)));
				end loop;
				-- OAM and VRAM.
				write(l, ' ' & hx(x_d) & ' ' & b01(x_non_vram) & b01(x_dma_ext) & b01(x_vram_oam)
					& b01(x_wuje) & b01(x_maka) & b01(x_mcs_n) & b01(x_mrd_n) & b01(x_mwr_n)
					& ' ' & hx(x_first_odd) & ' ' & hx(x_bus_even) & ' ' & hx(x_bus_odd)
					& ' ' & hx(x_vram_addr));
				-- The bus side.
				write(l, ' ' & b01(y_mreq) & b01(y_read) & b01(y_write) & b01(y_rd_n) & b01(y_wr_n)
					& b01(y_cs_n) & b01(y_unmapped) & b01(y_boot_sel)
					& ' ' & hx(y_a) & ' ' & hx(y_lat_addr) & ' ' & hx(y_lat_data)
					& ' ' & hx(y_p1) & ' ' & hx(y_jp_read) & ' ' & hx(y_jp_hist) & b01(y_jp_wake)
					& ' ' & hx(y_p10_13)
					& ' ' & hx(y_sb) & ' ' & b01(y_transfer) & b01(y_int_clock) & b01(y_half)
					& b01(y_ser_out) & b01(y_sck) & b01(y_sin) & ' ' & hx(std_ulogic_vector(y_ser_count))
					& ' ' & b01(latch_seen));
				-- The CPU core.
				write(l, ' ' & b01(q_halt) & b01(q_stop) & b01(q_halt_delayed) & b01(q_stop_delayed)
					& b01(q_pending_delayed) & b01(q_not_ready_1) & b01(q_not_ready_2) & b01(q_ime_state)
					& b01(q_ime) & b01(q_no_nmi) & b01(q_no_nmi_1) & b01(q_entering) & b01(q_int_entry)
					& b01(q_sel_reg_w) & b01(q_cb_next) & b01(q_cb_flag) & b01(q_cond_fail)
					& b01(q_z7_master) & b01(q_z7_delayed)
					& ' ' & hx(q_opcode) & ' ' & hx(q_ir_next) & ' ' & hx(q_mcyc)
					& ' ' & hx(q_a & q_flag_z & q_flag_n & q_flag_h & q_flag_c & "0000")
					& ' ' & hx(q_b & q_c) & ' ' & hx(q_d & q_e) & ' ' & hx(q_h & q_l) & ' ' & hx(q_w & q_z)
					& ' ' & hx(q_sph & q_spl) & ' ' & hx(q_pch & q_pcl)
					& ' ' & hx(q_pending) & ' ' & hx(q_db) & ' ' & hx(data_low));
				-- The APU.
				write(l, ' ' & b01(a_power) & b01(a_test_i) & b01(a_test_bit5) & b01(a_baza) & b01(a_ajer_i) & b01(a_calo) & b01(a_atep) & b01(a_cemo) & b01(a_cery) & b01(a_atyk) & b01(a_avok) & b01(a_jeso) & b01(a_bara) & b01(a_caru) & b01(a_bylu)
					& ' ' & hx(a_nr50) & ' ' & hx(a_nr51));
				write(l, ' ' & b01(c1_length_on) & b01(c1_trigger) & b01(c1_start) & b01(c1_triggered) & b01(c1_restart) & b01(c1_restarted) & b01(c1_running) & b01(c1_frozen) & b01(c1_active_i) & b01(c1_reload) & b01(c1_wave) & b01(c1_expired) & b01(c1_hz64) & b01(c1_tick) & b01(c1_at_limit) & b01(c1_env_stopped) & b01(c1_sweep_step) & b01(c1_shifting) & b01(c1_shift_done)
					& ' ' & hx(c1_sweep) & ' ' & hx(c1_duty) & ' ' & hx(c1_envelope) & ' ' & hx(c1_acc) & ' ' & hx(std_ulogic_vector(c1_counter)) & ' ' & hx(std_ulogic_vector(c1_step)) & ' ' & hx(std_ulogic_vector(c1_length)) & ' ' & hx(std_ulogic_vector(c1_period)) & ' ' & hx(std_ulogic_vector(c1_volume)) & ' ' & hx(std_ulogic_vector(c1_sweep_count)) & ' ' & hx(std_ulogic_vector(c1_shifts)) & ' ' & hx(c1_shifted) & ' ' & hx(c1_augend) & ' ' & hx(c1_addend));
				write(l, ' ' & b01(c2_length_on) & b01(c2_trigger) & b01(c2_start) & b01(c2_triggered) & b01(c2_restart) & b01(c2_restarted) & b01(c2_running) & b01(c2_frozen) & b01(c2_active_i) & b01(c2_reload) & b01(c2_wave) & b01(c2_expired) & b01(c2_hz64) & b01(c2_tick) & b01(c2_at_limit) & b01(c2_env_stopped)
					& ' ' & hx(c2_duty) & ' ' & hx(c2_envelope) & ' ' & hx(c2_frequency) & ' ' & hx(std_ulogic_vector(c2_counter)) & ' ' & hx(std_ulogic_vector(c2_step)) & ' ' & hx(std_ulogic_vector(c2_length)) & ' ' & hx(std_ulogic_vector(c2_period)) & ' ' & hx(std_ulogic_vector(c2_volume)));
				write(l, ' ' & b01(c3_dac_on_i) & b01(c3_length_on) & b01(c3_trigger) & b01(c3_start) & b01(c3_triggered) & b01(c3_restart) & b01(c3_restarted) & b01(c3_running) & b01(c3_frozen) & b01(c3_playing) & b01(c3_active_i) & b01(c3_reload) & b01(c3_wrapped) & b01(c3_expired) & b01(c3_busa) & b01(c3_bano) & b01(c3_fetch) & b01(c3_azet)
					& ' ' & hx(c3_level) & ' ' & hx(c3_frequency) & ' ' & hx(std_ulogic_vector(c3_counter)) & ' ' & hx(std_ulogic_vector(c3_position)) & ' ' & hx(std_ulogic_vector(c3_length)) & ' ' & hx(c3_bit_lines) & ' ' & hx(c3_sample_byte) & ' ');
				for i in 0 to 15 loop
					write(l, hx(c3_ram(i)));
				end loop;
				write(l, ' ' & b01(c4_length_on) & b01(c4_trigger) & b01(c4_start) & b01(c4_triggered) & b01(c4_restart) & b01(c4_restarted) & b01(c4_running) & b01(c4_frozen) & b01(c4_active_i) & b01(c4_divided) & b01(c4_joto) & b01(c4_expired) & b01(c4_hz64) & b01(c4_tick) & b01(c4_at_limit) & b01(c4_env_stopped)
					& ' ' & hx(c4_envelope) & ' ' & hx(c4_noise) & ' ' & hx(std_ulogic_vector(c4_divider)) & ' ' & hx(std_ulogic_vector(c4_prescaler)) & ' ' & hx(c4_lfsr) & ' ' & hx(std_ulogic_vector(c4_length)) & ' ' & hx(std_ulogic_vector(c4_period)) & ' ' & hx(std_ulogic_vector(c4_volume)));
				writeline(f, l);
			end if;
			ended := phase;
			rose  := (others => '0');
			latch_seen := y_addr_latch;
			mode0_rose := '0';
			for i in data'range loop
				data_low(i) := '1' when data(i) = '0' else '0';
			end loop;
		end loop;
	end process;
	else generate
		blocks_trace : process
		begin
			assert BLOCKS = "" report "BLOCKS traces the APU's channels: WITH_CHANNELS must be true" severity failure;
			wait;
		end process;
	end generate;

	cut_traces : if REVISION = DMG_B generate
		-- What the blocks show each other, seen when the accesses are (in the
		-- middle of phase H), from the M-cycle of the CPU's first access on.
		cut_trace : process
			alias phase      is << signal .gameboy_tb.u_gameboy.chip.u_chip.phase : phase_t >>;
			alias data_phase is << signal .gameboy_tb.u_gameboy.chip.u_chip.data_phase : std_ulogic >>;
			alias c          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.c : controls_t >>;
			alias flags      is << signal .gameboy_tb.u_gameboy.chip.u_chip.flags : std_ulogic_vector(4 downto 0) >>;
			alias tima       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_timer.tima : std_ulogic_vector(7 downto 0) >>;
			alias dma_run    is << signal .gameboy_tb.u_gameboy.chip.u_chip.dma_run : std_ulogic >>;
			alias ly         is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.ly_i : unsigned(7 downto 0) >>;
			alias stat_mode2 is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.stat_mode2 : std_ulogic >>;
			alias mode3      is << signal .gameboy_tb.u_gameboy.chip.u_chip.mode3 : std_ulogic >>;
			alias mode1      is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.mode1_i : std_ulogic >>;
			alias div        is << signal .gameboy_tb.u_gameboy.chip.u_chip.div : unsigned(15 downto 0) >>;
			alias oam_free   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam.free : std_ulogic >>;
			file f          : text;
			variable l      : line;
			variable mcycle : natural := 0;
			variable first  : boolean := true;
			variable seen   : boolean := false;
			variable mode   : std_ulogic_vector(1 downto 0);
			variable last_flags, flags_now : std_ulogic_vector(7 downto 0);
			variable last_tima, last_ly    : std_ulogic_vector(7 downto 0);
			variable last_mode             : std_ulogic_vector(1 downto 0);
			variable last_dma              : std_ulogic;
			variable last_oamb, last_vramb : std_ulogic;
			variable last_div              : unsigned(15 downto 0);

			procedure put(name : string; value : string) is
			begin
				write(l, integer'image(mcycle) & ' ' & name & ' ' & value);
				writeline(f, l);
			end procedure;
		begin
			if CUTS = "" then
				wait;
			end if;
			file_open(f, CUTS, write_mode);
			loop
				wait until phase = A or phase = H;
				if phase = A then
					mcycle := mcycle + 1;
				else
					wait for 60 ns;
					if not seen and data_phase = '1' and (c.fetch = '1' or c.mread = '1' or c.mwrite = '1') then
						seen   := true;
						mcycle := 0;
					end if;
					if seen then
						flags_now := "000" & to_x01(flags);
						mode      := (stat_mode2 or mode3) & (mode3 or mode1);
						if first or flags_now /= last_flags then
							put("IF", to_hstring(flags_now));
						end if;
						if first or to_x01(tima) /= last_tima then
							put("TIMA", to_hstring(to_x01(tima)));
						end if;
						if first or to_x01(dma_run) /= last_dma then
							put("DMA", to_string(to_x01(dma_run)));
						end if;
						if first or std_ulogic_vector(ly) /= last_ly then
							put("LY", to_hstring(std_ulogic_vector(ly)));
						end if;
						if first or to_x01(mode) /= last_mode then
							put("MODE", integer'image(to_integer(unsigned(to_01(mode)))));
						end if;
						if first or to_x01(not oam_free) /= last_oamb then
							put("OAMB", to_string(to_x01(not oam_free)));
						end if;
						if first or to_x01(mode3) /= last_vramb then
							put("VRAMB", to_string(to_x01(mode3)));
						end if;
						if first or div /= last_div + 1 then
							put("DIV", to_hstring(div));
						end if;
						first      := false;
						last_flags := flags_now;
						last_tima  := to_x01(tima);
						last_dma   := to_x01(dma_run);
						last_ly    := std_ulogic_vector(ly);
						last_mode  := to_x01(mode);
						last_div   := div;
						last_oamb  := to_x01(not oam_free);
						last_vramb := to_x01(mode3);
					end if;
				end if;
			end loop;
		end process;
	else generate
		-- What the blocks show each other, seen when the accesses are (in the
		-- middle of phase H), from the M-cycle of the CPU's first access on.
		cut_trace : process
			alias double_speed is << signal .gameboy_tb.u_gameboy.chip.u_chip.double_speed : std_ulogic >>;
			alias phase      is << signal .gameboy_tb.u_gameboy.chip.u_chip.phase : phase_t >>;
			alias data_phase is << signal .gameboy_tb.u_gameboy.chip.u_chip.data_phase : std_ulogic >>;
			alias c          is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_cpu.c : controls_t >>;
			alias flags      is << signal .gameboy_tb.u_gameboy.chip.u_chip.flags : std_ulogic_vector(4 downto 0) >>;
			alias tima       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_timer.tima : std_ulogic_vector(7 downto 0) >>;
			alias dma_run    is << signal .gameboy_tb.u_gameboy.chip.u_chip.dma_run : std_ulogic >>;
			alias ly_read    is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.ly_read : std_ulogic_vector(7 downto 0) >>;
			alias mode_read  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_lcd_timing.mode_read : std_ulogic_vector(1 downto 0) >>;
			alias mode3      is << signal .gameboy_tb.u_gameboy.chip.u_chip.mode3 : std_ulogic >>;
			alias div        is << signal .gameboy_tb.u_gameboy.chip.u_chip.div : unsigned(15 downto 0) >>;
			alias oam_free   is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_oam.free : std_ulogic >>;
			file f          : text;
			variable l      : line;
			variable first_at : delay_length := 0 ns;
			variable first  : boolean := true;
			variable seen   : boolean := false;
			variable mode   : std_ulogic_vector(1 downto 0);
			variable last_flags, flags_now : std_ulogic_vector(7 downto 0);
			variable last_tima, last_ly    : std_ulogic_vector(7 downto 0);
			variable last_mode             : std_ulogic_vector(1 downto 0);
			variable last_dma              : std_ulogic;
			variable last_oamb, last_vramb : std_ulogic;
			variable last_div              : unsigned(15 downto 0);

			procedure put(name : string; value : string) is
			begin
				write(l, mcycle_at(now - first_at) & ' ' & name & ' ' & value);
				writeline(f, l);
			end procedure;
		begin
			if CUTS = "" then
				wait;
			end if;
			file_open(f, CUTS, write_mode);
			loop
				wait until phase = H;
				wait for in_phase_h(double_speed);
				if not seen and data_phase = '1' and (c.fetch = '1' or c.mread = '1' or c.mwrite = '1') then
					seen     := true;
					first_at := now;
				end if;
				if seen then
					flags_now := "000" & to_x01(flags);
					mode      := mode_read;
					if first or flags_now /= last_flags then
						put("IF", to_hstring(flags_now));
					end if;
					if first or to_x01(tima) /= last_tima then
						put("TIMA", to_hstring(to_x01(tima)));
					end if;
					if first or to_x01(dma_run) /= last_dma then
						put("DMA", to_string(to_x01(dma_run)));
					end if;
					if first or ly_read /= last_ly then
						put("LY", to_hstring(ly_read));
					end if;
					if first or to_x01(mode) /= last_mode then
						put("MODE", integer'image(to_integer(unsigned(to_01(mode)))));
					end if;
					if first or to_x01(not oam_free) /= last_oamb then
						put("OAMB", to_string(to_x01(not oam_free)));
					end if;
					if first or to_x01(mode3) /= last_vramb then
						put("VRAMB", to_string(to_x01(mode3)));
					end if;
					if first or div /= last_div + 1 then
						put("DIV", to_hstring(div));
					end if;
					first      := false;
					last_flags := flags_now;
					last_tima  := to_x01(tima);
					last_dma   := to_x01(dma_run);
					last_ly    := ly_read;
					last_mode  := to_x01(mode);
					last_div   := div;
					last_oamb  := to_x01(not oam_free);
					last_vramb := to_x01(mode3);
				end if;
			end loop;
		end process;
	end generate;

	-- The pixels the LCD takes, and the tiles the fetcher starts.
	pixel_events : process
		alias tile_map  is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.tile_map_cycle : std_ulogic >>;
		alias in_window is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.in_window : std_ulogic >>;
		alias ly        is << signal .gameboy_tb.u_gameboy.chip.u_chip.ly : unsigned(7 downto 0) >>;
		alias lcdc      is << signal .gameboy_tb.u_gameboy.chip.u_chip.lcdc : std_ulogic_vector(7 downto 0) >>;
		alias wx        is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.wx : std_ulogic_vector(7 downto 0) >>;
		alias scy       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.scy : std_ulogic_vector(7 downto 0) >>;
		alias scx       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.scx : std_ulogic_vector(7 downto 0) >>;
		alias bgp       is << signal .gameboy_tb.u_gameboy.chip.u_chip.u_render.bgp : std_ulogic_vector(7 downto 0) >>;
		file f          : text;
		variable l      : line;
		variable x      : natural range 0 to 161 := 0;

		procedure event(what : string) is
		begin
			write(l, integer'image(to_integer(ly)) & ' ' & what & ' ' & integer'image(now / 119 ns));
			writeline(f, l);
		end procedure;

		procedure take_pixel is
		begin
			if x < 160 then
				event("P " & integer'image(x));
			end if;
			if x < 161 then
				x := x + 1;
			end if;
		end procedure;
	begin
		if PIXELS = "" then
			wait;
		end if;
		file_open(f, PIXELS, write_mode);
		loop
			wait until falling_edge(cp) or falling_edge(st) or rising_edge(cpl) or rising_edge(tile_map)
				or lcdc'event or wx'event or scy'event or scx'event or bgp'event
				or rising_edge(in_window);
			if falling_edge(cp) then
				take_pixel;
				if st = '1' then
					x := 0;
				end if;
			elsif falling_edge(st) or rising_edge(cpl) then
				take_pixel;
			elsif lcdc'event then
				event("W 40 " & to_hstring(lcdc));
			elsif wx'event then
				event("W 4B " & to_hstring(wx));
			elsif scy'event then
				event("W 42 " & to_hstring(scy));
			elsif scx'event then
				event("W 43 " & to_hstring(scx));
			elsif bgp'event then
				event("W 47 " & to_hstring(bgp));
			elsif rising_edge(in_window) then
				-- The window starts in the middle of a tile fetch: it restarts
				-- (a start before the fetch has its own fetch).
				if tile_map = '1' then
					event("T w");
				end if;
			elsif rising_edge(tile_map) then
				if in_window = '1' then
					event("T w");
				else
					event("T b");
				end if;
			end if;
		end loop;
	end process;

	-- The LCD, read as dmg-sim's vid_dump.sv reads it: the pixels of a line
	-- on the falls of CP (ST high at a fall starts the line again), on the
	-- fall of ST and on the rise of CPL, which latches the line; S starts
	-- the frame. A pixel is the DMG's 2 bits (a shade, 0 white) or the CGB's
	-- 15 (red in the low 5).
	lcd : process
		type line_t is array (0 to 159) of std_ulogic_vector(14 downto 0);
		type frame_t is array (0 to 143) of line_t;
		variable pixels  : line_t;
		variable frame   : frame_t;
		variable x       : natural range 0 to 160 := 0;
		-- 144: no frame is being taken, until S starts one.
		variable y       : natural range 0 to 144 := 144;
		variable enabled : boolean := false;
		variable shown   : natural := 0;
		file frames_file : text;

		procedure take_pixel is
		begin
			if x < 160 then
				pixels(x) := "0000000000000" & ld when REVISION = DMG_B else rgb;
				x := x + 1;
			end if;
		end procedure;

		procedure write_screen is
			file f     : text;
			variable l : line;
		begin
			file_open(f, SCREEN, write_mode);
			if REVISION = DMG_B then
				write(l, string'("P2 160 144 3"));
			else
				write(l, string'("P3 160 144 31"));
			end if;
			writeline(f, l);
			for row in frame'range loop
				for col in line_t'range loop
					if REVISION = DMG_B then
						write(l, 3 - to_integer(unsigned(to_01(frame(row)(col)(1 downto 0)))));
						write(l, ' ');
					else
						for channel in 0 to 2 loop
							write(l, to_integer(unsigned(to_01(frame(row)(col)(5 * channel + 4 downto 5 * channel)))));
							write(l, ' ');
						end loop;
					end if;
				end loop;
				writeline(f, l);
			end loop;
			file_close(f);
		end procedure;

		procedure write_frame is
			variable l : line;
		begin
			write(l, "frame " & integer'image(shown));
			writeline(frames_file, l);
			for row in frame'range loop
				for col in line_t'range loop
					if REVISION = DMG_B then
						write(l, to_integer(unsigned(to_01(frame(row)(col)(1 downto 0)))));
					else
						write(l, to_hstring('0' & to_01(frame(row)(col))));
					end if;
				end loop;
				writeline(frames_file, l);
			end loop;
			shown := shown + 1;
		end procedure;
	begin
		if SCREEN = "" and FRAMES = "" then
			wait;
		end if;
		if FRAMES /= "" then
			file_open(frames_file, FRAMES, write_mode);
		end if;
		loop
			wait until rising_edge(s) or falling_edge(cp) or falling_edge(st) or rising_edge(cpl) or falling_edge(cpl);
			if rising_edge(s) then
				y := 0;
			elsif falling_edge(cp) then
				take_pixel;
				if st = '1' then
					x := 0;
				end if;
			elsif falling_edge(st) then
				take_pixel;
			elsif rising_edge(cpl) then
				take_pixel;
				if enabled and y < 144 then
					-- Fewer than 160 pixels repeat, as in vid_dump.sv.
					for i in line_t'range loop
						frame(y)(i) := pixels(i mod x) when x > 0 else (others => '0');
					end loop;
					y := y + 1;
					-- The frame is whole when its last line is latched, not
					-- when the next one starts: the LCD may go off between.
					if y = 144 then
						if SCREEN /= "" then
							write_screen;
						end if;
						if FRAMES /= "" then
							write_frame;
						end if;
					end if;
				end if;
			elsif falling_edge(cpl) then
				-- The first frame after the LCD is turned on is not shown
				-- (nor does ceres's example write it): S does not start it,
				-- and its first line is not a whole one. A frame cut off
				-- when the LCD went off is not taken either. An LCD off
				-- between two falls of CPL goes unseen: the frame it cut
				-- off takes the next frame's lines, and is taken. (Turning
				-- the LCD on makes CPL fall too, from div(6) high to vclk.
				-- Ceres's examples do the same.)
				if cpg = '1' and not enabled then
					y := 144;
				end if;
				enabled := cpg = '1';
			end if;
		end loop;
	end process;
end architecture;
