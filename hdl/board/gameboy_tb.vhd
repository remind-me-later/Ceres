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
-- BLOCKS gets the divider, the timer and IF at the end of every phase, from
-- the CPU's first access on, for ceres-core's replay through its
-- transcriptions of them (netlist_replay.rs): the phase that ends; the
-- address, RD and WR, and the data bus; DIV's counter, TIMA, TMA and TAC;
-- the timer's overflow flip-flops (int_timer, bit7_before), its clock and
-- its write strobes of TMA and TAC; the interrupt requests, the
-- acknowledges, IF's flags and its read latch, IE, and the requests that
-- rose in the phase (also those that fell again before its end). Values in
-- hexadecimal, bits as 0 or 1.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use std.textio.all;
use std.env.finish;

use work.clocking.all;
use work.high_ram_types.all;
use work.sm83_controls.all;

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
	signal ld       : std_ulogic_vector(1 downto 0);
	signal so1, so2 : real;

	signal booted : boolean := false;

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

	procedure end_with(result : string) is
	begin
		report "result: " & result & " (" & now_ms & ")";
		write_file(RESULT_FILE, result);
		finish;
	end procedure;
begin
	u_gameboy : entity work.gameboy
		generic map (
			ROM => ROM, BOOTROM => BOOTROM, WITH_CHANNELS => WITH_CHANNELS, VRAM_ZERO => VRAM_ZERO
		)
		port map (
			sin => sin, sout => sout, sck => sck,
			cpg => cpg, cp => cp, cpl => cpl, fr => fr, st => st, s => s, ld => ld,
			so1 => so1, so2 => so2
		);

	-- The run's time limit, counted from the end of the reset.
	time_limit : process
		alias cpu_sys_reset is << signal .gameboy_tb.u_gameboy.u_chip.cpu_sys_reset : std_ulogic >>;
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
		alias opcode is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.opcode : std_ulogic_vector(7 downto 0) >>;
		alias b is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.u_registers.b : std_ulogic_vector(7 downto 0) >>;
		alias c is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.u_registers.c_reg : std_ulogic_vector(7 downto 0) >>;
		alias d is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.u_registers.d : std_ulogic_vector(7 downto 0) >>;
		alias e is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.u_registers.e : std_ulogic_vector(7 downto 0) >>;
		alias h is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.u_registers.h : std_ulogic_vector(7 downto 0) >>;
		alias l is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.u_registers.l : std_ulogic_vector(7 downto 0) >>;
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
		alias ram is << signal .gameboy_tb.u_gameboy.u_chip.u_high_ram.ram : ram_t >>;
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

	-- The CPU's accesses, seen in the middle of phase H: the M-cycle's
	-- address and the byte on the core's data bus, read or written. The
	-- phases run on through HALT, which stops the core's clocks (no
	-- data_phase then), so that M-cycles count time.
	accesses : process
		alias phase      is << signal .gameboy_tb.u_gameboy.u_chip.phase : phase_t >>;
		alias data_phase is << signal .gameboy_tb.u_gameboy.u_chip.data_phase : std_ulogic >>;
		alias c          is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.c : controls_t >>;
		alias address    is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.idu_in : std_ulogic_vector(15 downto 0) >>;
		alias db         is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.db : std_logic_vector(7 downto 0) >>;
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

	-- The divider, the timer and IF at the end of every phase: sampled as the
	-- next phase starts, before the flip-flops change (tco after the edge).
	blocks_trace : process
		alias phase       is << signal .gameboy_tb.u_gameboy.u_chip.phase : phase_t >>;
		alias data_phase  is << signal .gameboy_tb.u_gameboy.u_chip.data_phase : std_ulogic >>;
		alias c           is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.c : controls_t >>;
		alias addr        is << signal .gameboy_tb.u_gameboy.u_chip.addr : std_logic_vector(15 downto 0) >>;
		alias data        is << signal .gameboy_tb.u_gameboy.u_chip.data : std_logic_vector(7 downto 0) >>;
		alias cpu_rd      is << signal .gameboy_tb.u_gameboy.u_chip.cpu_rd : std_ulogic >>;
		alias cpu_wr      is << signal .gameboy_tb.u_gameboy.u_chip.cpu_wr : std_ulogic >>;
		alias div         is << signal .gameboy_tb.u_gameboy.u_chip.div : unsigned(15 downto 0) >>;
		alias tima        is << signal .gameboy_tb.u_gameboy.u_chip.u_timer.tima : std_ulogic_vector(7 downto 0) >>;
		alias tma         is << signal .gameboy_tb.u_gameboy.u_chip.u_timer.tma : std_ulogic_vector(7 downto 0) >>;
		alias tac         is << signal .gameboy_tb.u_gameboy.u_chip.u_timer.tac : std_ulogic_vector(2 downto 0) >>;
		alias int_timer   is << signal .gameboy_tb.u_gameboy.u_chip.u_timer.int_timer_i : std_ulogic >>;
		alias bit7_before is << signal .gameboy_tb.u_gameboy.u_chip.u_timer.bit7_before : std_ulogic >>;
		alias timer_clock is << signal .gameboy_tb.u_gameboy.u_chip.u_timer.timer_clock : std_ulogic >>;
		alias write_tma   is << signal .gameboy_tb.u_gameboy.u_chip.u_timer.write_tma : std_ulogic >>;
		alias write_tac   is << signal .gameboy_tb.u_gameboy.u_chip.u_timer.write_tac : std_ulogic >>;
		alias int_vbl     is << signal .gameboy_tb.u_gameboy.u_chip.int_vbl : std_ulogic >>;
		alias int_stat    is << signal .gameboy_tb.u_gameboy.u_chip.int_stat : std_ulogic >>;
		alias int_serial  is << signal .gameboy_tb.u_gameboy.u_chip.int_serial : std_ulogic >>;
		alias int_jp      is << signal .gameboy_tb.u_gameboy.u_chip.int_jp : std_ulogic >>;
		alias acks        is << signal .gameboy_tb.u_gameboy.u_chip.acks : std_ulogic_vector(4 downto 0) >>;
		alias flags       is << signal .gameboy_tb.u_gameboy.u_chip.flags : std_ulogic_vector(4 downto 0) >>;
		alias read_value  is << signal .gameboy_tb.u_gameboy.u_chip.u_interrupts.read_value : std_ulogic_vector(4 downto 0) >>;
		alias ie          is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.u_interrupts.ie : std_ulogic_vector(7 downto 0) >>;
		file f        : text;
		variable l    : line;
		variable seen : boolean := false;
		variable ended : phase_t;
		-- The requests that rose in the phase, also for less than its length.
		variable rose : std_ulogic_vector(4 downto 0) := (others => '0');

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
			wait on phase, int_vbl, int_stat, int_timer, int_serial, int_jp;
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
					& ' ' & hx(flags) & ' ' & hx(read_value) & ' ' & hx(ie) & ' ' & hx(rose));
				writeline(f, l);
			end if;
			ended := phase;
			rose  := (others => '0');
		end loop;
	end process;

	-- What the blocks show each other, seen when the accesses are (in the
	-- middle of phase H), from the M-cycle of the CPU's first access on.
	cut_trace : process
		alias phase      is << signal .gameboy_tb.u_gameboy.u_chip.phase : phase_t >>;
		alias data_phase is << signal .gameboy_tb.u_gameboy.u_chip.data_phase : std_ulogic >>;
		alias c          is << signal .gameboy_tb.u_gameboy.u_chip.u_cpu.c : controls_t >>;
		alias flags      is << signal .gameboy_tb.u_gameboy.u_chip.flags : std_ulogic_vector(4 downto 0) >>;
		alias tima       is << signal .gameboy_tb.u_gameboy.u_chip.u_timer.tima : std_ulogic_vector(7 downto 0) >>;
		alias dma_run    is << signal .gameboy_tb.u_gameboy.u_chip.dma_run : std_ulogic >>;
		alias ly         is << signal .gameboy_tb.u_gameboy.u_chip.u_lcd_timing.ly_i : unsigned(7 downto 0) >>;
		alias stat_mode2 is << signal .gameboy_tb.u_gameboy.u_chip.u_lcd_timing.stat_mode2 : std_ulogic >>;
		alias mode3      is << signal .gameboy_tb.u_gameboy.u_chip.mode3 : std_ulogic >>;
		alias mode1      is << signal .gameboy_tb.u_gameboy.u_chip.u_lcd_timing.mode1_i : std_ulogic >>;
		alias div        is << signal .gameboy_tb.u_gameboy.u_chip.div : unsigned(15 downto 0) >>;
		alias oam_free   is << signal .gameboy_tb.u_gameboy.u_chip.u_oam.free : std_ulogic >>;
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

	-- The pixels the LCD takes, and the tiles the fetcher starts.
	pixel_events : process
		alias tile_map  is << signal .gameboy_tb.u_gameboy.u_chip.u_render.tile_map_cycle : std_ulogic >>;
		alias in_window is << signal .gameboy_tb.u_gameboy.u_chip.u_render.in_window : std_ulogic >>;
		alias ly        is << signal .gameboy_tb.u_gameboy.u_chip.ly : unsigned(7 downto 0) >>;
		alias lcdc      is << signal .gameboy_tb.u_gameboy.u_chip.lcdc : std_ulogic_vector(7 downto 0) >>;
		alias wx        is << signal .gameboy_tb.u_gameboy.u_chip.u_render.wx : std_ulogic_vector(7 downto 0) >>;
		alias scy       is << signal .gameboy_tb.u_gameboy.u_chip.u_render.scy : std_ulogic_vector(7 downto 0) >>;
		alias scx       is << signal .gameboy_tb.u_gameboy.u_chip.u_render.scx : std_ulogic_vector(7 downto 0) >>;
		alias bgp       is << signal .gameboy_tb.u_gameboy.u_chip.u_render.bgp : std_ulogic_vector(7 downto 0) >>;
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
	-- the frame.
	lcd : process
		type line_t is array (0 to 159) of std_ulogic_vector(1 downto 0);
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
				pixels(x) := ld;
				x := x + 1;
			end if;
		end procedure;

		procedure write_screen is
			file f     : text;
			variable l : line;
		begin
			file_open(f, SCREEN, write_mode);
			write(l, string'("P2 160 144 3"));
			writeline(f, l);
			for row in frame'range loop
				for col in line_t'range loop
					write(l, 3 - to_integer(unsigned(to_01(frame(row)(col)))));
					write(l, ' ');
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
					write(l, to_integer(unsigned(to_01(frame(row)(col)))));
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
						frame(y)(i) := pixels(i mod x) when x > 0 else "00";
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
				-- when the LCD went off is not taken either.
				if cpg = '1' and not enabled then
					y := 144;
				end if;
				enabled := cpg = '1';
			end if;
		end loop;
	end process;
end architecture;
