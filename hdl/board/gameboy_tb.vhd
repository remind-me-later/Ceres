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
-- BUS_TRACE gets a line per memory access of the CPU, as ceres's example
-- bus_trace writes them (../compare-ceres.sh): the M-cycle, counted from
-- the CPU's first access, R or W, the address and the byte, in hexadecimal.

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
		BUS_TRACE   : string := ""
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
			ROM => ROM, BOOTROM => BOOTROM
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
		variable y       : natural range 0 to 144 := 0;
		variable enabled : boolean := false;
		variable first   : boolean := true;   -- the frame after the LCD is turned on
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
					-- The first one after the LCD is turned on is not shown
					-- (nor does ceres's example write it): its first line is
					-- not a whole one.
					if y = 144 then
						if first then
							first := false;
						else
							if SCREEN /= "" then
								write_screen;
							end if;
							if FRAMES /= "" then
								write_frame;
							end if;
						end if;
					end if;
				end if;
			elsif falling_edge(cpl) then
				if cpg = '1' and not enabled then
					first := true;
				end if;
				enabled := cpg = '1';
			end if;
		end loop;
	end process;
end architecture;
