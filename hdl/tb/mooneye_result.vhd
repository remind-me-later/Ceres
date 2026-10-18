-- SPDX-License-Identifier: MIT

-- The result of a Mooneye test: the test ends with LD B,B (opcode 40), with
-- the Fibonacci numbers 3, 5, 8, 13, 21 and 34 in B, C, D, E, H and L if it
-- passed. It looks once the boot ROM is done (BOOTED), since the boot ROM
-- runs LD B,B too.
--
-- It reads the CPU's registers by external names (<< signal ... >>),
-- relative to the design that instantiates it, which must call the chip
-- `dut` and instantiate it first.

library ieee;
use ieee.std_logic_1164.all;
use work.tb_util.all;

entity mooneye_result is
  generic (
    ENABLED     : boolean;
    RESULT_FILE : string
  );
  port (booted : in boolean);
end entity;

architecture sim of mooneye_result is
  -- The opcode register, and the registers B, C, D, E, H and L, bit by bit.
  alias op0 is << signal ^.dut.u_cpu.n_opcode0 : std_ulogic >>;
  alias op1 is << signal ^.dut.u_cpu.n_opcode1 : std_ulogic >>;
  alias op2 is << signal ^.dut.u_cpu.n_opcode2 : std_ulogic >>;
  alias op3 is << signal ^.dut.u_cpu.n_opcode3 : std_ulogic >>;
  alias op4 is << signal ^.dut.u_cpu.n_opcode4 : std_ulogic >>;
  alias op5 is << signal ^.dut.u_cpu.n_opcode5 : std_ulogic >>;
  alias op6 is << signal ^.dut.u_cpu.n_opcode6 : std_ulogic >>;
  alias op7 is << signal ^.dut.u_cpu.n_opcode7 : std_ulogic >>;
  alias b0 is << signal ^.dut.u_cpu.reg_bc.n_reg_b0 : std_ulogic >>;
  alias b1 is << signal ^.dut.u_cpu.reg_bc.n_reg_b1 : std_ulogic >>;
  alias b2 is << signal ^.dut.u_cpu.reg_bc.n_reg_b2 : std_ulogic >>;
  alias b3 is << signal ^.dut.u_cpu.reg_bc.n_reg_b3 : std_ulogic >>;
  alias b4 is << signal ^.dut.u_cpu.reg_bc.n_reg_b4 : std_ulogic >>;
  alias b5 is << signal ^.dut.u_cpu.reg_bc.n_reg_b5 : std_ulogic >>;
  alias b6 is << signal ^.dut.u_cpu.reg_bc.n_reg_b6 : std_ulogic >>;
  alias b7 is << signal ^.dut.u_cpu.reg_bc.n_reg_b7 : std_ulogic >>;
  alias c0 is << signal ^.dut.u_cpu.reg_bc.n_reg_c0 : std_ulogic >>;
  alias c1 is << signal ^.dut.u_cpu.reg_bc.n_reg_c1 : std_ulogic >>;
  alias c2 is << signal ^.dut.u_cpu.reg_bc.n_reg_c2 : std_ulogic >>;
  alias c3 is << signal ^.dut.u_cpu.reg_bc.n_reg_c3 : std_ulogic >>;
  alias c4 is << signal ^.dut.u_cpu.reg_bc.n_reg_c4 : std_ulogic >>;
  alias c5 is << signal ^.dut.u_cpu.reg_bc.n_reg_c5 : std_ulogic >>;
  alias c6 is << signal ^.dut.u_cpu.reg_bc.n_reg_c6 : std_ulogic >>;
  alias c7 is << signal ^.dut.u_cpu.reg_bc.n_reg_c7 : std_ulogic >>;
  alias d0 is << signal ^.dut.u_cpu.reg_de.n_reg_d0 : std_ulogic >>;
  alias d1 is << signal ^.dut.u_cpu.reg_de.n_reg_d1 : std_ulogic >>;
  alias d2 is << signal ^.dut.u_cpu.reg_de.n_reg_d2 : std_ulogic >>;
  alias d3 is << signal ^.dut.u_cpu.reg_de.n_reg_d3 : std_ulogic >>;
  alias d4 is << signal ^.dut.u_cpu.reg_de.n_reg_d4 : std_ulogic >>;
  alias d5 is << signal ^.dut.u_cpu.reg_de.n_reg_d5 : std_ulogic >>;
  alias d6 is << signal ^.dut.u_cpu.reg_de.n_reg_d6 : std_ulogic >>;
  alias d7 is << signal ^.dut.u_cpu.reg_de.n_reg_d7 : std_ulogic >>;
  alias e0 is << signal ^.dut.u_cpu.reg_de.n_reg_e0 : std_ulogic >>;
  alias e1 is << signal ^.dut.u_cpu.reg_de.n_reg_e1 : std_ulogic >>;
  alias e2 is << signal ^.dut.u_cpu.reg_de.n_reg_e2 : std_ulogic >>;
  alias e3 is << signal ^.dut.u_cpu.reg_de.n_reg_e3 : std_ulogic >>;
  alias e4 is << signal ^.dut.u_cpu.reg_de.n_reg_e4 : std_ulogic >>;
  alias e5 is << signal ^.dut.u_cpu.reg_de.n_reg_e5 : std_ulogic >>;
  alias e6 is << signal ^.dut.u_cpu.reg_de.n_reg_e6 : std_ulogic >>;
  alias e7 is << signal ^.dut.u_cpu.reg_de.n_reg_e7 : std_ulogic >>;
  alias h0 is << signal ^.dut.u_cpu.reg_hl.n_reg_h0 : std_ulogic >>;
  alias h1 is << signal ^.dut.u_cpu.reg_hl.n_reg_h1 : std_ulogic >>;
  alias h2 is << signal ^.dut.u_cpu.reg_hl.n_reg_h2 : std_ulogic >>;
  alias h3 is << signal ^.dut.u_cpu.reg_hl.n_reg_h3 : std_ulogic >>;
  alias h4 is << signal ^.dut.u_cpu.reg_hl.n_reg_h4 : std_ulogic >>;
  alias h5 is << signal ^.dut.u_cpu.reg_hl.n_reg_h5 : std_ulogic >>;
  alias h6 is << signal ^.dut.u_cpu.reg_hl.n_reg_h6 : std_ulogic >>;
  alias h7 is << signal ^.dut.u_cpu.reg_hl.n_reg_h7 : std_ulogic >>;
  alias l0 is << signal ^.dut.u_cpu.reg_hl.n_reg_l0 : std_ulogic >>;
  alias l1 is << signal ^.dut.u_cpu.reg_hl.n_reg_l1 : std_ulogic >>;
  alias l2 is << signal ^.dut.u_cpu.reg_hl.n_reg_l2 : std_ulogic >>;
  alias l3 is << signal ^.dut.u_cpu.reg_hl.n_reg_l3 : std_ulogic >>;
  alias l4 is << signal ^.dut.u_cpu.reg_hl.n_reg_l4 : std_ulogic >>;
  alias l5 is << signal ^.dut.u_cpu.reg_hl.n_reg_l5 : std_ulogic >>;
  alias l6 is << signal ^.dut.u_cpu.reg_hl.n_reg_l6 : std_ulogic >>;
  alias l7 is << signal ^.dut.u_cpu.reg_hl.n_reg_l7 : std_ulogic >>;
begin
  process is
    impure function opcode return std_logic_vector is
    begin
      return op7 & op6 & op5 & op4 & op3 & op2 & op1 & op0;
    end function;
    variable rb, rc, rd, re, rh, rl : std_logic_vector(7 downto 0);
  begin
    if not ENABLED then
      wait;
    end if;
    loop
      wait on op0, op1, op2, op3, op4, op5, op6, op7;
      -- The bits of the opcode register settle at different times, so 40
      -- can show for a moment between two other opcodes (from 00 to C3):
      -- it counts if it holds, as an opcode does for at least one M-cycle.
      if booted and opcode = x"40" then
        wait for 200 ns;
      end if;
      if booted and opcode = x"40" then
        -- Let the instruction finish.
        wait for 2 us;
        rb := b7 & b6 & b5 & b4 & b3 & b2 & b1 & b0;
        rc := c7 & c6 & c5 & c4 & c3 & c2 & c1 & c0;
        rd := d7 & d6 & d5 & d4 & d3 & d2 & d1 & d0;
        re := e7 & e6 & e5 & e4 & e3 & e2 & e1 & e0;
        rh := h7 & h6 & h5 & h4 & h3 & h2 & h1 & h0;
        rl := l7 & l6 & l5 & l4 & l3 & l2 & l1 & l0;
        if to_x01(rb & rc & rd & re & rh & rl) = x"0305080D1522" then
          finish(RESULT_FILE, "pass");
        else
          finish(RESULT_FILE, "fail B=" & hex(rb) & " C=" & hex(rc) & " D=" & hex(rd) &
                 " E=" & hex(re) & " H=" & hex(rh) & " L=" & hex(rl));
        end if;
      end if;
    end loop;
  end process;
end architecture;
