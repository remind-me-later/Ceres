-- Verilog semantics for the gate-level model.
--
-- The cell models and their delays come from dmg-sim, which runs them with
-- Icarus Verilog. These functions let the generated VHDL behave the same:
-- Verilog's delay selection, drive strengths, switch and tri-state gates,
-- and its operators on unknown values.

library ieee;
use ieee.std_logic_1164.all;

package verilog is
  -- Drive strength of a gate output (Verilog: supply, strong, pull, weak,
  -- highz). std_logic keeps two of them: strong 0/1, and weak L/H for pull
  -- and weak.
  type strength is (supply, strong, pull, weak, highz);

  -- A delay in ns rounded to the 1 ps precision of dmg-sim's timescale.
  function to_t(ns : real) return delay_length;
  -- cond ? a : b on reals (VHDL-2008 has no conditional expression).
  function rcond(c : boolean; a, b : real) return real;

  -- The value a gate drives with strengths (s1 for a 1, s0 for a 0). The
  -- gates below return H and L for Verilog's "1 or z" and "0 or z" (when
  -- their control is unknown): these stay weak, or vanish with highz.
  function drive(v : std_ulogic; s1, s0 : strength) return std_ulogic;
  -- Tri-state gates: bufif0/bufif1 drive the data when the control is 0/1.
  function bufif0_v(d, c : std_ulogic) return std_ulogic;
  function bufif1_v(d, c : std_ulogic) return std_ulogic;
  function notif0_v(d, c : std_ulogic) return std_ulogic;
  function notif1_v(d, c : std_ulogic) return std_ulogic;
  -- Switches pass the input with its strength; the resistive ones reduce a
  -- strong value to a weak one.
  function nmos_v(d, c : std_ulogic) return std_ulogic;
  function pmos_v(d, c : std_ulogic) return std_ulogic;
  function rnmos_v(d, c : std_ulogic) return std_ulogic;
  function rpmos_v(d, c : std_ulogic) return std_ulogic;

  -- Delay of a gate or continuous assignment, selected by the new value:
  -- rise, fall, turn-off; the smallest one to an unknown value.
  function gdly(v : std_ulogic; r, f, z : delay_length) return delay_length;
  -- Delay of a module path for the change pv -> nv: rise, fall, and the
  -- turn-off delays from 0 and from 1.
  function pdly(pv, nv : std_ulogic; r, f, z0, z1 : delay_length) return delay_length;
  -- Icarus times a module path from the last change of its source: the part
  -- of delay d left when age has passed since then (at least 0).
  function aged(d : delay_length; age : time) return delay_length;

  -- ==, != and === on single bits, and ?: with an unknown condition.
  function veq(a, b : std_ulogic) return std_ulogic;
  function vne(a, b : std_ulogic) return std_ulogic;
  function vceq(a, b : std_ulogic) return std_ulogic;
  function vmux(sel, a, b : std_ulogic) return std_ulogic;

  -- Verilog edges: posedge includes 0->x and x->1.
  function is_posedge(pv, nv : std_ulogic) return boolean;
  function is_negedge(pv, nv : std_ulogic) return boolean;
end package;

package body verilog is
  function to_t(ns : real) return delay_length is
  begin
    return integer(ns * 1000.0) * 1 ps;
  end function;

  function rcond(c : boolean; a, b : real) return real is
  begin
    if c then return a; else return b; end if;
  end function;

  function level(s : strength; one : boolean) return std_ulogic is
  begin
    case s is
      when supply | strong => if one then return '1'; else return '0'; end if;
      when pull | weak     => if one then return 'H'; else return 'L'; end if;
      when highz           => return 'Z';
    end case;
  end function;

  function drive(v : std_ulogic; s1, s0 : strength) return std_ulogic is
  begin
    case v is
      when '1'       => return level(s1, true);
      when '0'       => return level(s0, false);
      when 'H'       => if s1 = highz then return 'Z'; else return 'H'; end if;
      when 'L'       => if s0 = highz then return 'Z'; else return 'L'; end if;
      when 'Z'       => return 'Z';
      when others =>
        if s1 = highz and s0 = highz then
          return 'Z';
        elsif s1 >= pull and s0 >= pull then
          return 'W';
        else
          return 'X';
        end if;
    end case;
  end function;

  function bufif1_v(d, c : std_ulogic) return std_ulogic is
  begin
    case to_x01(c) is
      when '1' => return to_x01(d);
      when '0' => return 'Z';
      when others =>
        case to_x01(d) is
          when '1'    => return 'H';
          when '0'    => return 'L';
          when others => return 'X';
        end case;
    end case;
  end function;

  function bufif0_v(d, c : std_ulogic) return std_ulogic is
  begin
    return bufif1_v(d, not to_x01(c));
  end function;

  function notif1_v(d, c : std_ulogic) return std_ulogic is
  begin
    return bufif1_v(not to_x01(d), c);
  end function;

  function notif0_v(d, c : std_ulogic) return std_ulogic is
  begin
    return bufif1_v(not to_x01(d), not to_x01(c));
  end function;

  function nmos_v(d, c : std_ulogic) return std_ulogic is
  begin
    case to_x01(c) is
      when '1' =>
        if d = 'U' or d = '-' then return 'X'; else return d; end if;
      when '0' => return 'Z';
      when others =>
        -- Icarus resolves an unknown switch against the keeper of the
        -- net as unknown.
        if d = 'Z' then return 'Z'; else return 'X'; end if;
    end case;
  end function;

  function pmos_v(d, c : std_ulogic) return std_ulogic is
  begin
    return nmos_v(d, not to_x01(c));
  end function;

  function weaken(v : std_ulogic) return std_ulogic is
  begin
    case v is
      when '1' | 'H' => return 'H';
      when '0' | 'L' => return 'L';
      when 'Z'       => return 'Z';
      when others    => return 'W';
    end case;
  end function;

  function rnmos_v(d, c : std_ulogic) return std_ulogic is
  begin
    return weaken(nmos_v(d, c));
  end function;

  function rpmos_v(d, c : std_ulogic) return std_ulogic is
  begin
    return weaken(pmos_v(d, c));
  end function;

  function gdly(v : std_ulogic; r, f, z : delay_length) return delay_length is
  begin
    case v is
      when '1' | 'H' => return r;
      when '0' | 'L' => return f;
      when 'Z'       => return z;
      when others    => return minimum(r, minimum(f, z));
    end case;
  end function;

  function pdly(pv, nv : std_ulogic; r, f, z0, z1 : delay_length) return delay_length is
  begin
    case nv is
      when '1' | 'H' => return r;
      when '0' | 'L' => return f;
      when 'Z' =>
        case pv is
          when '0' | 'L' => return z0;
          when '1' | 'H' => return z1;
          when others    => return maximum(z0, z1);
        end case;
      when others =>
        case pv is
          when '0' | 'L' => return minimum(r, z0);
          when '1' | 'H' => return minimum(f, z1);
          when 'Z'       => return minimum(r, f);
          when others    => return minimum(minimum(r, f), minimum(z0, z1));
        end case;
    end case;
  end function;

  function aged(d : delay_length; age : time) return delay_length is
  begin
    if d > age then return d - age; else return 0 fs; end if;
  end function;

  function veq(a, b : std_ulogic) return std_ulogic is
  begin
    if to_x01(a) = 'X' or to_x01(b) = 'X' then return 'X';
    elsif to_x01(a) = to_x01(b) then return '1';
    else return '0';
    end if;
  end function;

  function vne(a, b : std_ulogic) return std_ulogic is
  begin
    return not veq(a, b);
  end function;

  function vceq(a, b : std_ulogic) return std_ulogic is
  begin
    if to_x01z(a) = to_x01z(b) then return '1'; else return '0'; end if;
  end function;

  function vmux(sel, a, b : std_ulogic) return std_ulogic is
  begin
    case to_x01(sel) is
      when '1' => return a;
      when '0' => return b;
      when others =>
        if to_x01z(a) = to_x01z(b) then return a; else return 'X'; end if;
    end case;
  end function;

  function is_posedge(pv, nv : std_ulogic) return boolean is
    constant p : std_ulogic := to_x01z(pv);
    constant n : std_ulogic := to_x01z(nv);
  begin
    return (p = '0' and n /= '0') or (p /= '1' and p /= '0' and n = '1');
  end function;

  function is_negedge(pv, nv : std_ulogic) return boolean is
    constant p : std_ulogic := to_x01z(pv);
    constant n : std_ulogic := to_x01z(nv);
  begin
    return (p = '1' and n /= '1') or (p /= '1' and p /= '0' and n = '0');
  end function;
end package body;

-------------------------------------------------------------------------------
-- Bus keeper: holds the last value driven on a tri-state net, weakly
-- (dmg-sim's keeper.sv).
--
-- Verilog has two strengths below strong: a pull-up (pull) wins over a
-- keeper (weak). std_logic has one, where the two conflict as W; the keeper
-- then lets go, and takes the value of the pull-up.

library ieee;
use ieee.std_logic_1164.all;

entity keeper is
  port (n : inout std_logic);
end entity;

architecture sim of keeper is
begin
  process (n) is
  begin
    case n is
      when 'Z'       => null;
      when '1' | 'H' => n <= 'H';
      when '0' | 'L' => n <= 'L';
      when 'W'       => n <= 'Z';
      when others    => n <= 'W';
    end case;
  end process;
end architecture;
