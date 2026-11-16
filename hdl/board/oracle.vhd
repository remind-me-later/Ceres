-- SPDX-License-Identifier: MIT
--
-- Reads the samples that ../oracle/probe_phases.sv writes from dmg-sim,
-- one phase at a time, to drive a block of the model with the inputs
-- dmg-sim had, and to compare the block's outputs with dmg-sim's.
--
-- A testbench calls next_phase, then drives the inputs (input), and at
-- the end of the phase compares the outputs (check). Each mismatch is
-- reported; after max_errors of them the simulation stops.

library ieee;
use ieee.std_logic_1164.all;

use std.textio.all;

use work.clocking.all;

package oracle is
	-- The length of a phase: dmg-sim's crystal toggles every 119 ns.
	constant T_phase : time := 119 ns;

	type oracle_t is protected
		procedure open_file(path : string; max_errors : natural := 20);

		-- Reads the samples of the next phase. Returns false at the end of
		-- the file.
		impure function next_phase return boolean;

		-- The number of the current phase.
		impure function phase return natural;

		-- The value of a signal at the end of the current phase.
		impure function value(name : string) return string;
		impure function bit_value(name : string) return std_ulogic;
		impure function vector_value(name : string) return std_ulogic_vector;
		impure function phase_value return phase_t;

		-- Compares a value with dmg-sim's. The bits that dmg-sim has as x
		-- (unknown since power-on) match any value.
		procedure check(name : string; actual : string);

		-- Compares a bus with dmg-sim's, except the bits that dmg-sim has as
		-- x: two drivers that disagree, which only a model of both shows. Each
		-- such phase is reported once, as a note.
		procedure check_bus(name : string; actual : string);

		-- Prints the number of phases and mismatches, and ends the
		-- simulation, with a failure if there were mismatches.
		procedure finish;

		impure function errors return natural;
	end protected;

	-- A value as dmg-sim writes it: 0, 1, x or z.
	function image(v : std_ulogic) return string;
	function image(v : std_ulogic_vector) return string;
	function image(p : phase_t) return string;
end package;

package body oracle is
	function image(v : std_ulogic) return string is
	begin
		case v is
			when '0' | 'L' => return "0";
			when '1' | 'H' => return "1";
			when 'Z' => return "z";
			when others => return "x";
		end case;
	end function;

	function image(v : std_ulogic_vector) return string is
		variable s : string(1 to v'length);
		variable i : positive := 1;
	begin
		for k in v'range loop
			s(i to i) := image(v(k));
			i := i + 1;
		end loop;
		return s;
	end function;

	function image(p : phase_t) return string is
		constant letters : string(1 to 8) := "ABCDEFGH";
	begin
		return letters(phase_t'pos(p) + 1 to phase_t'pos(p) + 1);
	end function;

	function to_std(c : character) return std_ulogic is
	begin
		case c is
			when '0' => return '0';
			when '1' => return '1';
			when 'z' => return 'Z';
			when others => return 'X';
		end case;
	end function;

	type oracle_t is protected body
		constant max_signals : positive := 1024;

		type names_t is array (1 to max_signals) of line;

		file f : text;

		variable names, values : names_t;
		variable count         : natural := 0;
		variable current       : integer := -1;
		variable pending       : line;     -- the next line, not yet applied
		variable pending_phase : integer := -1;
		variable at_end        : boolean := false;
		variable mismatches    : natural := 0;
		variable conflicts     : natural := 0;
		variable max_mismatch  : natural := 20;

		impure function find(name : string) return natural is
		begin
			for i in 1 to count loop
				if names(i).all = name then
					return i;
				end if;
			end loop;
			return 0;
		end function;

		procedure set(name : string; v : string) is
			variable i : natural := find(name);
		begin
			if i = 0 then
				count := count + 1;
				i := count;
				names(i) := new string'(name);
			else
				deallocate(values(i));
			end if;
			values(i) := new string'(v);
		end procedure;

		-- Reads the next word of l, up to a space or the end.
		procedure read_word(l : inout line; word : out line) is
			variable c  : character;
			variable ok : boolean;
			variable w  : line := new string'("");
			variable t  : line;
		begin
			loop
				read(l, c, ok);
				exit when not ok or c /= ' ';
			end loop;
			while ok and c /= ' ' loop
				t := new string'(w.all & c);
				deallocate(w);
				w := t;
				read(l, c, ok);
			end loop;
			word := w;
		end procedure;

		-- Reads the next line into pending.
		procedure read_pending is
			variable l    : line;
			variable p    : integer;
			variable ok   : boolean;
		begin
			if endfile(f) then
				at_end := true;
				return;
			end if;
			readline(f, l);
			read(l, p, ok);
			assert ok report "bad line in the oracle's file" severity failure;
			pending_phase := p;
			deallocate(pending);
			pending := l;
		end procedure;

		procedure open_file(path : string; max_errors : natural := 20) is
		begin
			file_open(f, path, read_mode);
			max_mismatch := max_errors;
			read_pending;
		end procedure;

		impure function next_phase return boolean is
			variable name, v : line;
		begin
			if at_end then
				return false;
			end if;
			current := current + 1;
			while not at_end and pending_phase = current loop
				-- pending holds " <name> <value>"
				read_word(pending, name);
				read_word(pending, v);
				set(name.all, v.all);
				deallocate(name);
				deallocate(v);
				read_pending;
			end loop;
			return true;
		end function;

		impure function phase return natural is
		begin
			return current;
		end function;

		impure function value(name : string) return string is
			variable i : natural := find(name);
		begin
			assert i /= 0 report "dmg-sim has no signal " & name severity failure;
			return values(i).all;
		end function;

		impure function bit_value(name : string) return std_ulogic is
			constant v : string := value(name);
		begin
			return to_std(v(v'left));
		end function;

		impure function vector_value(name : string) return std_ulogic_vector is
			constant v : string := value(name);
			variable r : std_ulogic_vector(v'length - 1 downto 0);
		begin
			for i in v'range loop
				r(v'right - i) := to_std(v(i));
			end loop;
			return r;
		end function;

		impure function phase_value return phase_t is
			constant v : string := value("phase");
		begin
			for p in phase_t loop
				if image(p) = v then
					return p;
				end if;
			end loop;
			report "dmg-sim's phase is " & v severity failure;
			return H;
		end function;

		procedure check(name : string; actual : string) is
			constant expected : string := value(name);
			variable masked   : string(actual'range) := actual;
		begin
			-- What dmg-sim does not know (x: never set since power-on), any value
			-- matches.
			if actual'length = expected'length then
				for i in expected'range loop
					if expected(i) = 'x' then
						masked(actual'left + i - expected'left) := 'x';
					end if;
				end loop;
			end if;
			if masked /= expected then
				mismatches := mismatches + 1;
				report "phase " & integer'image(current) & " (" & value("phase") & "): " & name & " is "
					& actual & ", dmg-sim " & expected severity error;
				if mismatches >= max_mismatch then
					report "too many mismatches" severity failure;
				end if;
			end if;
		end procedure;

		procedure check_bus(name : string; actual : string) is
			constant expected : string := value(name);
			variable masked   : string(actual'range) := actual;
			variable conflict : boolean := false;
		begin
			for i in expected'range loop
				if expected(i) = 'x' then
					masked(actual'left + i - expected'left) := 'x';
					conflict := true;
				end if;
			end loop;
			if conflict then
				conflicts := conflicts + 1;
				report "phase " & integer'image(current) & " (" & value("phase") & "): " & name & " is "
					& actual & ", dmg-sim " & expected & " (conflict)" severity note;
			end if;
			check(name, masked);
		end procedure;

		procedure finish is
		begin
			if mismatches = 0 then
				report integer'image(current + 1) & " phases, no mismatch, "
					& integer'image(conflicts) & " phases with conflicts";
				std.env.finish;
			else
				report integer'image(current + 1) & " phases, " & integer'image(mismatches) & " mismatches"
					severity failure;
			end if;
		end procedure;

		impure function errors return natural is
		begin
			return mismatches;
		end function;
	end protected body;
end package body;
