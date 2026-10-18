-- SPDX-License-Identifier: CC-BY-SA-4.0

-- A conflict between OAM DMA and the reading of OAM. On the chip, both
-- drive the data lines of the OAM at once, which the gate model cannot
-- resolve; dmg-sim's testbench (solve_oam_conflict) forces the data lines
-- of both OAM halves, 20 ns after the conflict starts and until it ends,
-- to values computed once (Icarus evaluates its forces once). OAM half B,
-- bit 3, has an alternative result on some devices, which dmg-sim leaves
-- off.
--
-- It reaches into the chip by external names (<< signal ... >>), relative
-- to the design that instantiates it, which must call the chip `dut` and
-- instantiate it first.

library ieee;
use ieee.std_logic_1164.all;

entity oam_conflict is
end entity;

architecture sim of oam_conflict is
  alias oam_oe       is << signal ^.dut.ppu_oam.n_oam_oe : std_ulogic >>;
  alias dma_addr_ext is << signal ^.dut.n_dma_addr_ext : std_ulogic >>;
  -- Which bits the reading of OAM selects.
  alias ralo is << signal ^.dut.n_ralo : std_ulogic >>;
  alias tune is << signal ^.dut.n_tune : std_ulogic >>;
  alias sera is << signal ^.dut.n_sera : std_ulogic >>;
  alias tenu is << signal ^.dut.n_tenu : std_ulogic >>;
  alias sysa is << signal ^.dut.n_sysa : std_ulogic >>;
  alias sugy is << signal ^.dut.n_sugy : std_ulogic >>;
  alias tube is << signal ^.dut.n_tube : std_ulogic >>;
  alias syzo is << signal ^.dut.n_syzo : std_ulogic >>;
  -- What each OAM half outputs.
  alias dout_a is << signal ^.dut.ppu_oam.u_oam_a.dout : std_logic_vector(7 downto 0) >>;
  alias dout_b is << signal ^.dut.ppu_oam.u_oam_b.dout : std_logic_vector(7 downto 0) >>;
  -- The data lines of the two halves (active low).
  alias a_d0 is << signal ^.dut.n_oam_a_d0_n : std_logic >>;
  alias a_d1 is << signal ^.dut.n_oam_a_d1_n : std_logic >>;
  alias a_d2 is << signal ^.dut.n_oam_a_d2_n : std_logic >>;
  alias a_d3 is << signal ^.dut.n_oam_a_d3_n : std_logic >>;
  alias a_d4 is << signal ^.dut.n_oam_a_d4_n : std_logic >>;
  alias a_d5 is << signal ^.dut.n_oam_a_d5_n : std_logic >>;
  alias a_d6 is << signal ^.dut.n_oam_a_d6_n : std_logic >>;
  alias a_d7 is << signal ^.dut.n_oam_a_d7_n : std_logic >>;
  alias b_d0 is << signal ^.dut.n_oam_b_d0_n : std_logic >>;
  alias b_d1 is << signal ^.dut.n_oam_b_d1_n : std_logic >>;
  alias b_d2 is << signal ^.dut.n_oam_b_d2_n : std_logic >>;
  alias b_d3 is << signal ^.dut.n_oam_b_d3_n : std_logic >>;
  alias b_d4 is << signal ^.dut.n_oam_b_d4_n : std_logic >>;
  alias b_d5 is << signal ^.dut.n_oam_b_d5_n : std_logic >>;
  alias b_d6 is << signal ^.dut.n_oam_b_d6_n : std_logic >>;
  alias b_d7 is << signal ^.dut.n_oam_b_d7_n : std_logic >>;
begin
  process is
    variable sel : std_ulogic_vector(0 to 7);
  begin
    wait until (to_x01(oam_oe) and to_x01(dma_addr_ext)) = '1';
    wait for 20 ns;
    sel := (not to_x01(ralo), not to_x01(tune), not to_x01(sera), not to_x01(tenu),
            not to_x01(sysa), not to_x01(sugy), not to_x01(tube), not to_x01(syzo));
    a_d0 <= force sel(0) and to_x01(dout_a(7));
    a_d1 <= force sel(1) and to_x01(dout_a(6));
    a_d2 <= force sel(2) and to_x01(dout_a(5));
    a_d3 <= force sel(3) and to_x01(dout_a(4));
    a_d4 <= force sel(4) and to_x01(dout_a(3));
    a_d5 <= force sel(5) and to_x01(dout_a(2));
    a_d6 <= force sel(6) and to_x01(dout_a(1));
    a_d7 <= force sel(7) and to_x01(dout_a(0));
    b_d0 <= force sel(0) and to_x01(dout_b(0));
    b_d1 <= force to_x01(dout_b(1));
    b_d2 <= force sel(2) and to_x01(dout_b(2));
    b_d3 <= force sel(3) and to_x01(dout_b(3));
    b_d4 <= force sel(4) and to_x01(dout_b(4));
    b_d5 <= force sel(5) and to_x01(dout_b(5));
    b_d6 <= force sel(6) and to_x01(dout_b(6));
    b_d7 <= force sel(7) and to_x01(dout_b(7));
    wait until (to_x01(oam_oe) and to_x01(dma_addr_ext)) /= '1';
    a_d0 <= release; a_d1 <= release; a_d2 <= release; a_d3 <= release;
    a_d4 <= release; a_d5 <= release; a_d6 <= release; a_d7 <= release;
    b_d0 <= release; b_d1 <= release; b_d2 <= release; b_d3 <= release;
    b_d4 <= release; b_d5 <= release; b_d6 <= release; b_d7 <= release;
  end process;
end architecture;
