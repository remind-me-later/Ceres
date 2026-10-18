-- SPDX-License-Identifier: MIT
--
-- The SM83's IDU: it adds 1 to the 16 bits of its input, or subtracts 1
-- (PC after a fetch, SP for PUSH and POP, INC rr, DEC rr, LD (HL+)). For
-- JR, it adjusts the high byte of PC: by +1 or -1 when the ALU's addition
-- of e to PCL carried or borrowed. Its input is also the address that the
-- core puts out.
--
-- The carry chain is precharged while data_phase is low: the result is
-- only right in the second half of the M-cycle (E to H). Before, only bit 0
-- is flipped.
--
-- @hw die: dmg-schematics, category idu (IDU_CTL, IDU_MUX_L, IDU_MUX_H,
-- IDU_BIT0, IDU_BIT123456, IDU_BIT7, IDU_XOR_L, IDU_XOR_H)

library ieee;
use ieee.std_logic_1164.all;

use work.sm83_decoder.all;
use work.sm83_controls.all;

entity sm83_idu is
	port (
		data_phase : in  std_ulogic;  -- E to H
		s          : in  steps_t;
		c          : in  controls_t;
		z7         : in  std_ulogic;  -- the sign of e (JR)
		carry7     : in  std_ulogic;  -- the carry of PCL + e (JR)
		idu_in     : in  std_ulogic_vector(15 downto 0);
		idu_out    : out std_ulogic_vector(15 downto 0)
	);
end entity;

architecture rtl of sm83_idu is
	signal inc, dec, at8 : std_ulogic;
	signal ena           : std_ulogic_vector(15 downto 0);  -- the carry goes through
	signal flip          : std_ulogic_vector(15 downto 0);  -- the bits that change
begin
	-- JR: +1 on the high byte when e is positive and PCL + e carried, -1
	-- when e is negative and it did not; the chain then starts at bit 8.
	-- @hw die: IDU_CTL
	inc <= c.idu_inc or (s.jr_m1 and carry7 and not z7);
	dec <= c.idu_dec or (s.jr_m1 and not carry7 and z7);
	at8 <= s.jr_m1 and (carry7 xor z7);

	-- A bit passes the carry on when it is 1 (counting up) or 0 (down).
	-- @hw die: IDU_MUX_L, IDU_MUX_H
	carry : for i in 0 to 15 generate
		ena(i) <= inc when idu_in(i) = '1' else dec;
	end generate;

	-- @hw die: IDU_BIT0 (or, zero, aoi), IDU_BIT123456, IDU_BIT7
	chain : process (all)
		variable carry_on : std_ulogic;
	begin
		flip(0) <= inc or dec;
		carry_on := data_phase and ena(0);
		for i in 1 to 7 loop
			flip(i)  <= carry_on;
			carry_on := carry_on and ena(i);
		end loop;
		carry_on := data_phase and (carry_on or at8);
		for i in 8 to 15 loop
			flip(i)  <= carry_on;
			carry_on := carry_on and ena(i);
		end loop;
	end process;

	-- @hw die: IDU_XOR_L, IDU_XOR_H
	idu_out <= idu_in xor flip;
end architecture;
