-- SPDX-License-Identifier: MIT
--
-- The timer: TIMA (FF05), TMA (FF06) and TAC (FF07).
--
-- @hw die: dmg-schematics, category timer (sheet "FF05-FF07 timer");
-- checked against dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

use work.clocking.all;

entity timer is
	port (
		clk    : in std_ulogic;
		phase  : in phase_t;
		clk_t4 : in std_ulogic;
		reset2 : in std_ulogic;

		div : in unsigned(15 downto 0);

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : in std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		int_timer : out std_ulogic  -- TIMA overflowed: request the interrupt
	);
end entity;

architecture rtl of timer is
	signal tima, tma        : std_ulogic_vector(7 downto 0) := (others => '0');
	signal tac              : std_ulogic_vector(2 downto 0) := (others => '0');
	signal write_tima       : std_ulogic;
	signal write_tma        : std_ulogic;
	signal write_tac        : std_ulogic;
	signal tac_select       : std_ulogic_vector(1 downto 0) := (others => '0');
	signal timer_clock      : std_ulogic;
	signal load             : std_ulogic;
	signal load_value       : std_ulogic_vector(7 downto 0);
	signal bit7_before      : std_ulogic := '0';
	signal int_timer_i      : std_ulogic := '0';
begin
	-- @hw die: TOPE TYJU SARA (nand4), RYFO
	write_tima <= '1' when cpu_wr = '1' and addr = x"FF05" else '0';
	write_tma  <= '1' when cpu_wr = '1' and addr = x"FF06" else '0';
	write_tac  <= '1' when cpu_wr = '1' and addr = x"FF07" else '0';

	-- TMA and TAC take the data bus when the CPU's write ends.
	-- @hw die: SABU NYKE MURU TYVA TYRU SUFY PETO SETA (dffr), clocked by TYJU
	tma_ff : process (write_tma, reset2)
	begin
		if reset2 = '1' then
			tma <= (others => '0');
		elsif falling_edge(write_tma) then
			tma <= to_x01(data) after tco;
		end if;
	end process;

	-- Bit 2 enables the timer, bits 0 and 1 select its clock.
	-- @hw die: SOPU SAMY SABO (dffr), clocked by SARA
	tac_ff : process (write_tac, reset2)
	begin
		if reset2 = '1' then
			tac <= (others => '0');
		elsif falling_edge(write_tac) then
			tac <= to_x01(data(2 downto 0)) after tco;
		end if;
	end process;

	-- The timer's clock is the selected bit of the divider, gated by the
	-- enable bit, and TIMA counts when it falls: when the divider's bit
	-- falls, but also when TAC changes the bit or clears the enable, and
	-- when a write to DIV clears the divider.
	--
	-- When a write changes the enable and the select at once, the enable
	-- arrives first: the select goes through two multiplexers. Enabling
	-- the timer while the old selected bit is 1 and the new one is 0 thus
	-- makes the clock rise and fall at once, and TIMA counts. tac_select is
	-- the select a moment (a delta cycle) after TAC.
	-- @hw die: TEKO UKAP TECY (muxi), SOGU
	-- @fact timer-enable-race
	tac_select <= tac(1 downto 0);

	with tac_select select timer_clock <=
		tac(2) and div(7) when "00",   --   4096 Hz
		tac(2) and div(1) when "01",   -- 262144 Hz
		tac(2) and div(3) when "10",   --  65536 Hz
		tac(2) and div(5) when others; --  16384 Hz

	-- TIMA follows its load value for as long as it loads: during a reset,
	-- for the M-cycle after an overflow (from TMA), and while the CPU writes
	-- it, except in G and H (from the data bus). Otherwise it counts.
	-- @hw die: MEXU MUZU MEKE, PUXY NERO NADA REPA ROLU RUGY PYMA PAGU (nor2),
	-- ROKE PETU NYKU SOCE SALA SYRU REFU RATO (muxi)
	load       <= reset2 or int_timer_i or (write_tima and not clk_t4);
	load_value <= (others => '0') when reset2 = '1' else
	              to_x01(data) when write_tima = '1' else
	              tma;

	-- TIMA's bits are toggle flip-flops, each toggled when the one before it
	-- falls (a ripple counter), and transparent while they load.
	-- @hw die: REGA POVY PERU RATE RUBY RAGE PEDA NUGA (tffnl)
	tima_counter : process (load, load_value, timer_clock)
	begin
		if load = '1' then
			tima <= load_value;
		elsif falling_edge(timer_clock) then
			tima <= std_ulogic_vector(unsigned(tima) + 1) after tco;
		end if;
	end process;

	-- An overflow, when bit 7 falls, shows at the start of the next M-cycle:
	-- int_timer is then high for an M-cycle, which reloads TIMA from TMA and
	-- requests the interrupt.
	-- @hw die: NYDU MOBA (dffr), MERY, clocked by BOGA
	overflow : process (clk, reset2, load)
	begin
		if reset2 = '1' then
			int_timer_i <= '0';
			bit7_before <= '0';
		else
			if starts(clk, phase, A) then
				int_timer_i <= bit7_before and not tima(7) after tco;
			end if;
			if load = '1' then
				bit7_before <= '0';
			elsif starts(clk, phase, A) then
				bit7_before <= tima(7) after tco;
			end if;
		end if;
	end process;
	int_timer <= int_timer_i;

	-- @hw die: TEDA TUBY SORA, SOKU..PUSO SETE..SAPU RYLA ROTE SUPE (not_if1)
	data_out <= tima when cpu_rd = '1' and addr = x"FF05" else
	        tma when cpu_rd = '1' and addr = x"FF06" else
	        "ZZZZZ" & tac when cpu_rd = '1' and addr = x"FF07" else
	        (others => 'Z');
end architecture;
