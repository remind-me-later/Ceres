-- SPDX-License-Identifier: MIT
--
-- The joypad register P1 (FF00) and the pins P10 to P15.
--
-- @hw die: dmg-schematics, category joypad (sheet "FF00 JOYP"), and the
-- bits of FF00 that only test mode uses (category test); checked against
-- dmg-sim (../check/chip_tb.vhd).

library ieee;
use ieee.std_logic_1164.all;

use work.clocking.all;

entity joypad is
	port (
		clk      : in std_ulogic;
		phase    : in phase_t;
		clk_1mhz : in std_ulogic;
		reset2   : in std_ulogic;

		-- The CPU's bus
		addr   : in    std_ulogic_vector(15 downto 0);
		data   : in std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd : in    std_ulogic;
		cpu_wr : in    std_ulogic;

		-- Pins: P10 to P13 are inputs with pull-ups, which a pressed button
		-- connects to P14 or P15; FF00 bits 4 and 5 drive P14 and P15 low.
		p10_13 : inout std_logic_vector(3 downto 0);
		p14    : out   std_logic;
		p15    : out   std_logic;

		-- The test register FF60: bit 0 makes the other bits of FF00 drive
		-- the pins P10 to P13, SIN and SOUT.
		test_pins : in  std_ulogic;
		sin_drive : out std_ulogic;
		sout_test : out std_ulogic;

		int_jp     : out std_ulogic;  -- a button was pressed
		cpu_wakeup : out std_ulogic   -- a button is pressed: STOP ends
	);
end entity;

architecture rtl of joypad is
	signal write_p1, read_p1 : std_ulogic;
	signal p1                : std_ulogic_vector(7 downto 0) := (others => '0');
	signal inputs            : std_ulogic_vector(3 downto 0);
	signal read_value        : std_ulogic_vector(3 downto 0) := (others => '0');
	signal pressed           : std_ulogic;
	signal history           : std_ulogic_vector(3 downto 0) := (others => '0');
	signal wakeup            : std_ulogic := '0';
begin
	-- @hw die: ATOZ ACAT, ANAP AMUS AKUG BYKO
	write_p1 <= '1' when cpu_wr = '1' and addr = x"FF00" else '0';
	read_p1  <= '1' when cpu_rd = '1' and addr = x"FF00" else '0';

	-- FF00 is written when the CPU's write ends. Bits 4 and 5 select the
	-- buttons; the others only matter in test mode.
	-- @hw die: JUTE KECY JALE KYME KELY COFY KUKO KERU (dffr), clocked by ATOZ
	p1_ff : process (write_p1, reset2)
	begin
		if reset2 = '1' then
			p1 <= (others => '0');
		elsif falling_edge(write_p1) then
			p1 <= to_x01(data) after tco;
		end if;
	end process;

	-- P14 and P15 are low while their bit is 0, and float otherwise, except
	-- in test mode, where they are high.
	-- @hw die: P14 P15 (pad_out_diff), KARU CELA
	p14 <= '0' when p1(4) = '0' else '1' when test_pins = '1' else 'Z';
	p15 <= '0' when p1(5) = '0' else '1' when test_pins = '1' else 'Z';

	-- P10 to P13 have pull-ups; test mode drives them from bits 0 to 3.
	-- @hw die: P10..P13 (pad_bidir_pu), KOLE KYTO KYHU KORY, KYBU KABU KASY KALE
	pins : for i in 0 to 3 generate
		p10_13(i) <= p1(i) when test_pins = '1' else 'H';
	end generate;
	inputs <= to_x01(p10_13);

	-- Test mode: bit 7 drives SIN, bit 6 SOUT.
	-- @hw die: KORE KYWE, KUKO
	sin_drive <= p1(7);
	sout_test <= p1(6);

	-- A read sees the pins as they were when it started.
	-- @hw die: KEVU KAPA KEJA KOLO (dlatch), BYZO
	read_latch : process (all)
	begin
		if read_p1 = '0' then
			read_value <= inputs;
		end if;
	end process;

	-- @hw die: KEMA KURO KUVE JEKU KOCE CUDY (not_if0)
	data_out <= "ZZ" & p1(5 downto 4) & read_value when read_p1 = '1' else (others => 'Z');

	-- A button is pressed when one of P10 to P13 is low. The interrupt is
	-- requested when that is so at the start of an M-cycle and was so three
	-- M-cycles earlier.
	-- @hw die: KERY, BATU ACEF AGEM APUG (dffr), clocked by BOGA; ASOK
	pressed <= not (inputs(0) and inputs(1) and inputs(2) and inputs(3));

	debounce : process (clk, reset2)
	begin
		if reset2 = '1' then
			history <= (others => '0');
		elsif starts(clk, phase, A) then
			history <= history(2 downto 0) & pressed after tco;
		end if;
	end process;
	int_jp <= history(0) and history(3);

	-- @hw die: AWOB (dlatch), open while BOGA is high: in phase A, and while
	-- the oscillator is stopped
	wakeup_latch : process (all)
	begin
		if clk_1mhz = '1' then
			wakeup <= pressed;
		end if;
	end process;
	cpu_wakeup <= wakeup;
end architecture;
