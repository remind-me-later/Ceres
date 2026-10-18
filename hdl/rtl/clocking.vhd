-- SPDX-License-Identifier: MIT
--
-- The time base of the model.
--
-- The crystal runs at 4 MiHz, and every edge of it, rising or falling,
-- starts a new phase: eight phases, A to H, make an M-cycle of the CPU
-- (1 MiHz). The clock inside the chip is high in phases A, C, E and G.
-- The letters follow GateBoy's names for the phases.
--
-- Every register of the model changes at the start of a phase, that is
-- at an edge of the clock, so each one is written as
--
--   if starts(clk, phase, A) then ... end if;
--
-- At that edge, phase still holds the phase that ends: the clocks block
-- changes it a moment (a delta cycle) later. Latches, which follow their
-- input for as long as they are open, and asynchronous resets are
-- written as in the hardware.
--
-- A flip-flop's output changes tco (1 ns) after its clock's edge, and the
-- gates, latches and clocks derived from other clocks react at once, as in
-- the chip, where a flip-flop takes longer than a few gates: every flip-flop
-- clocked by an edge, or by a clock that follows from it, sees the values
-- from before the edge.

library ieee;
use ieee.std_logic_1164.all;

package clocking is
	type phase_t is (A, B, C, D, E, F, G, H);

	-- The clock-to-output delay of the flip-flops.
	constant tco : time := 1 ns;

	-- The phase after p.
	function next_phase(p : phase_t) return phase_t;

	-- True at the edge of clk that starts phase p.
	function starts(signal clk : std_ulogic; signal phase : phase_t; p : phase_t) return boolean;
end package;

package body clocking is
	function next_phase(p : phase_t) return phase_t is
	begin
		if p = H then
			return A;
		else
			return phase_t'succ(p);
		end if;
	end function;

	function starts(signal clk : std_ulogic; signal phase : phase_t; p : phase_t) return boolean is
	begin
		return clk'event and next_phase(phase) = p;
	end function;
end package body;
