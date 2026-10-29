-- SPDX-License-Identifier: MIT
--
-- SVBK (FF70), the bank of the work RAM at D000 to DFFF, and the three bank
-- lines to the work RAM chip, which the CGB's work RAM needs beside the
-- cartridge bus's A0 to A11: 32 KiB in eight banks of 4 KiB, bank 0 at
-- C000 to CFFF, the bank SVBK selects at D000 to DFFF (bank 1 for 0), and
-- the same at E000 to FDFF.
--
-- The lines follow the address pins, so OAM DMA from D000 to DFFF reads the
-- selected bank as the CPU does.
--
-- @hw inferred: no netlist of the CGB's chip is known. The bank's rules are
-- Pan Docs' ("FF70 — SVBK"); that the boot ROM can write SVBK in the DMG's
-- mode is SameBoy's (it selects bank 1 after switching to the DMG's mode).
-- Where the bank lines are (pins to an external RAM, as here, or inside
-- the chip) the CPU does not see.
-- @rule cgb-work-ram-banks

library ieee;
use ieee.std_logic_1164.all;

entity wram_bank is
	port (
		reset2   : in std_ulogic;
		cgb_mode : in std_ulogic;
		boot_on  : in std_ulogic;  -- the boot ROM is mapped

		-- The CPU's bus
		addr     : in  std_ulogic_vector(15 downto 0);
		data     : in  std_logic_vector(7 downto 0);
		data_out : out std_logic_vector(7 downto 0);
		cpu_rd   : in  std_ulogic;
		cpu_wr   : in  std_ulogic;

		a12_pin   : in  std_ulogic;                     -- A12 on the cartridge bus
		bank_pins : out std_ulogic_vector(2 downto 0)   -- A12 to A14 of the work RAM
	);
end entity;

architecture rtl of wram_bank is
	signal write_svbk : std_ulogic;
	signal svbk       : std_ulogic_vector(2 downto 0) := (others => '0');
begin
	write_svbk <= '1' when cpu_wr = '1' and addr = x"FF70" else '0';

	svbk_reg : process (write_svbk, reset2)
	begin
		if reset2 = '1' then
			svbk <= (others => '0');
		elsif falling_edge(write_svbk) and (cgb_mode = '1' or boot_on = '1') then
			svbk <= to_x01(data(2 downto 0));
		end if;
	end process;

	bank_pins <= "000" when to_x01(a12_pin) = '0' else
	             "001" when svbk = "000" else
	             svbk;

	data_out <= "11111" & svbk when cpu_rd = '1' and addr = x"FF70" and cgb_mode = '1' else
	            (others => 'Z');
end architecture;
