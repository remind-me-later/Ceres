-- SPDX-License-Identifier: MIT
--
-- The divider and DIV (FF04): a 16-bit counter of M-cycles, whose bits
-- 6 to 13 the CPU reads at FF04, and whose other bits clock the timer,
-- the APU, the serial port and the power-on wait.
--
-- @hw die: dmg-schematics, category clocks (sheet "FF04 DIV"); checked
-- against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity divider is
	port (
		clk   : in std_ulogic;
		phase : in phase_t;

		reset     : in std_ulogic;  -- the /RESET pin
		osc_ena   : in std_ulogic;  -- the oscillator runs
		test_fast : in std_ulogic;  -- bit 1 of the test register FF60

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : inout std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		div : out unsigned(15 downto 0)
	);
end entity;

architecture rtl of divider is
	signal count     : unsigned(15 downto 0) := (others => '0');
	signal reset_div : std_ulogic;
begin
	-- Writing FF04 clears the whole counter, for as long as the write lasts;
	-- so do /RESET and a stopped oscillator.
	-- @hw die: TAPE, UFOL
	reset_div <= '1' when reset = '1' or osc_ena = '0' or (cpu_wr = '1' and addr = x"FF04") else '0';

	-- The counter counts at the start of each M-cycle. Its bits are
	-- flip-flops that each toggle when the one before falls (a ripple
	-- counter), and they settle within the phase. The test register makes
	-- bits 6 to 15 count M-cycles on their own.
	-- @hw die: UKUP UFOR UNER TERO UNYK TAMA UGOT TULU TUGO TOFE TERU SOLA
	-- SUBU TEKA UKET UPOF (dffr), ULUR (mux), clocked by BOGA
	counter : process (clk, reset_div)
	begin
		if reset_div = '1' then
			count <= (others => '0');
		elsif starts(clk, phase, A) then
			if test_fast = '1' then
				count(5 downto 0)  <= count(5 downto 0) + 1;
				count(15 downto 6) <= count(15 downto 6) + 1;
			else
				count <= count + 1;
			end if;
		end if;
	end process;
	div <= count;

	-- @hw die: TAGY, TAWU TAKU TEMU TUSE UPUG SEPU SAWA TATU (not_if1)
	data <= std_logic_vector(count(13 downto 6)) when cpu_rd = '1' and addr = x"FF04" else (others => 'Z');
end architecture;
