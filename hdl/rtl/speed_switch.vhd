-- SPDX-License-Identifier: MIT
--
-- KEY1 (FF4D) and the switch between the normal speed and double speed.
--
-- A STOP with KEY1's bit 0 set switches the speed: STOP stops the
-- oscillator, as on the DMG, the speed changes while it is stopped, and
-- this block wakes the CPU core a moment later, as a joypad line does on
-- the DMG. The rest is the DMG's: the stopped oscillator has cleared the
-- divider, and the CPU core stays halted until the divider's last bit is
-- set (../clocks.vhd, the power-on wait), 2^15 M-cycles at the new speed,
-- or until an interrupt is requested.
--
-- @hw inferred: KEY1's bits are Pan Docs' ("FF4D — KEY1"); that the switch
-- is a STOP woken at once is a guess, which gives the wait ceres and
-- SameBoy have (0x20008 T-cycles at the new speed: 2^17, and the two
-- M-cycles of the CPU core's wake) and the reset of DIV. How long the
-- oscillator stays stopped is fitted to ceres, whose times pass Gambatte's
-- speedchange tests (CGB-C): longer into double speed than out of it.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity speed_switch is
	generic (
		-- The periods of the crystal (8 MiHz) from the stop of the
		-- oscillator to the wake, into double speed and out of it
		WAKE_INTO_DOUBLE : positive := 16;
		WAKE_INTO_NORMAL : positive := 4
	);
	port (
		ck1      : in std_ulogic;  -- the crystal, which runs while the oscillator is stopped
		reset2   : in std_ulogic;
		cgb_mode : in std_ulogic;
		osc_ena  : in std_ulogic;  -- the oscillator runs (low in STOP)

		-- The CPU's bus
		addr     : in  std_ulogic_vector(15 downto 0);
		data     : in  std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd   : in  std_ulogic;
		cpu_wr   : in  std_ulogic;

		double_speed : out std_ulogic;  -- KEY1 bit 7
		wake         : out std_ulogic   -- ends the STOP of a switch
	);
end entity;

architecture rtl of speed_switch is
	signal write_key1   : std_ulogic;
	signal armed        : std_ulogic := '0';  -- KEY1 bit 0
	signal speed        : std_ulogic := '0';  -- KEY1 bit 7
	signal switching    : std_ulogic := '0';
	signal wake_after   : positive;
	signal count        : natural range 0 to WAKE_INTO_DOUBLE + 8 := 0;
begin
	write_key1 <= '1' when cpu_wr = '1' and addr = x"FF4D" else '0';

	-- The switch: when STOP stops the oscillator with bit 0 set, the speed
	-- changes and bit 0 clears.
	key1 : process (write_key1, osc_ena, reset2)
	begin
		if reset2 = '1' then
			armed     <= '0';
			speed     <= '0';
			switching <= '0';
		else
			if falling_edge(write_key1) and cgb_mode = '1' then
				armed <= to_x01(data(0));
			end if;
			if falling_edge(osc_ena) and armed = '1' then
				speed     <= not speed;
				armed     <= '0';
				switching <= '1';
			end if;
			if rising_edge(osc_ena) then
				switching <= '0';
			end if;
		end if;
	end process;
	double_speed <= speed;

	-- The wake comes a few periods of the crystal after the stop, and lasts
	-- a few more, until the CPU core has seen its clock again.
	wake_after <= WAKE_INTO_DOUBLE when speed = '1' else WAKE_INTO_NORMAL;

	wake_counter : process (ck1)
	begin
		if rising_edge(ck1) and (switching = '1' or count /= 0) then
			if count = wake_after + 8 then
				count <= 0;
			else
				count <= count + 1;
			end if;
		end if;
	end process;
	wake <= '1' when count >= wake_after else '0';

	data_out <= speed & "111111" & armed when cpu_rd = '1' and addr = x"FF4D" and cgb_mode = '1' else
	            (others => 'Z');
end architecture;
