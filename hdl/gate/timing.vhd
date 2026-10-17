-- Delay model of the DMG-CPU B and of its SM83 core.
--
-- Ported from dmg-sim (dmg_cpu_b/timing-default.sv and
-- sm83/timing-default.sv, Copyright Michael Singer, CC-BY-SA 4.0). The
-- delays are Elmore delays of RC networks: the resistance of the driving
-- transistor from its width, and the capacitance of the wire from its
-- length plus the gates it drives. The constants were fitted by dmg-sim to
-- measurements of a real DMG-CPU B. Delays are in ns.

package dmg_timing is
  -- Wire parasitics
  constant Rprime_wire    : real := 0.001;    -- ohm/um
  constant Cprime_wire    : real := 0.14e-15; -- F/um

  -- Transistor drive constants
  constant Lref_um        : real := 1.0;      -- reference channel length for kR
  constant Leff_um        : real := 1.2;      -- guessed for DMG technology
  constant kR_nmos_ref    : real := 3.26e4;   -- ohm*um for Lref
  constant gamma_pn       : real := 1.88;     -- PMOS = gamma * NMOS resistance

  constant Cint_stage_F   : real := 0.8e-15;  -- F intrinsic capacitance added after each stage

  constant Cgate_per_um_F : real := 0.71e-15; -- F capacitance per transistor gate width

  -- Scale for converting Electric VLSI layout units to um
  constant L_unit         : real := 0.454;

  constant T_Z_min        : real := 0.005;    -- 5 ps

  constant kR_nmos        : real := kR_nmos_ref * (Leff_um / Lref_um);

  function R_nmos_ohm(W_um : real) return real;
  function R_pmos_ohm(Wp_um : real) return real;
  function C_wire_F(L_um : real) return real;
  function R_wire_ohm(L_um : real) return real;
  function C_gate_F(W_um : real) return real;
  function tpd_elmore_s(L_um, R_drv_ohm : real; C_extra_F : real := 0.0) return real;
  function tpd_elmore(L_um, R_drv_ohm : real; C_extra_F : real := 0.0) return real;
  function tpd_z(T_plh : real := 0.0; T_phl : real := 0.0; T_min : real := T_Z_min) return real;
end package;

package body dmg_timing is
  function R_nmos_ohm(W_um : real) return real is
  begin
    return kR_nmos / W_um;
  end function;

  function R_pmos_ohm(Wp_um : real) return real is
  begin
    return gamma_pn * kR_nmos / Wp_um;
  end function;

  function C_wire_F(L_um : real) return real is
  begin
    return Cprime_wire * L_um;
  end function;

  function R_wire_ohm(L_um : real) return real is
  begin
    return Rprime_wire * L_um;
  end function;

  function C_gate_F(W_um : real) return real is
  begin
    return Cgate_per_um_F * W_um;
  end function;

  function tpd_elmore_s(L_um, R_drv_ohm : real; C_extra_F : real := 0.0) return real is
    constant Cw : real := C_wire_F(L_um);
    constant Rw : real := R_wire_ohm(L_um);
  begin
    return 0.69 * R_drv_ohm * (Cw + Cint_stage_F + C_extra_F) + 0.38 * Rw * Cw; -- seconds
  end function;

  function tpd_elmore(L_um, R_drv_ohm : real; C_extra_F : real := 0.0) return real is
  begin
    return tpd_elmore_s(L_um, R_drv_ohm, C_extra_F) * 1.0e9;
  end function;

  function tpd_z(T_plh : real := 0.0; T_phl : real := 0.0; T_min : real := T_Z_min) return real is
    variable T_pd : real;
  begin
    if T_plh >= T_phl then T_pd := T_plh; else T_pd := T_phl; end if;
    if 0.3 * T_pd > T_min then return 0.3 * T_pd; else return T_min; end if;
  end function;
end package body;

-------------------------------------------------------------------------------

package sm83_timing is
  -- Wire parasitics
  constant Rprime_wire    : real := 1.08;     -- ohm/um
  constant Cprime_wire    : real := 0.18e-15; -- F/um

  -- Transistor drive constants
  constant Lref_um        : real := 1.0;
  constant Leff_um        : real := 1.2;
  constant kR_nmos_ref    : real := 3.26e4;
  constant gamma_pn       : real := 1.88;

  constant Cint_stage_F   : real := 0.8e-15;

  constant Cgate_per_um_F : real := 0.71e-15;

  constant L_unit         : real := 0.432;

  constant T_Z_min        : real := 0.005;

  constant kR_nmos        : real := kR_nmos_ref * (Leff_um / Lref_um);

  function R_nmos_ohm(W_um : real) return real;
  function R_pmos_ohm(Wp_um : real) return real;
  function C_wire_F(L_um : real) return real;
  function R_wire_ohm(L_um : real) return real;
  function C_gate_F(W_um : real) return real;
  function tpd_elmore_s_rc(Rw, Cw, R_drv_ohm : real; C_extra_F : real := 0.0) return real;
  function tpd_elmore_s(L_um, R_drv_ohm : real; C_extra_F : real := 0.0) return real;
  function tpd_elmore(L_um, R_drv_ohm : real; C_extra_F : real := 0.0) return real;
  function tpd_elmore_rc(Rw, Cw, R_drv_ohm : real; C_extra_F : real := 0.0) return real;
  function tpd_z(T_plh : real := 0.0; T_phl : real := 0.0; T_min : real := T_Z_min) return real;
end package;

package body sm83_timing is
  function R_nmos_ohm(W_um : real) return real is
  begin
    return kR_nmos / W_um;
  end function;

  function R_pmos_ohm(Wp_um : real) return real is
  begin
    return gamma_pn * kR_nmos / Wp_um;
  end function;

  function C_wire_F(L_um : real) return real is
  begin
    return Cprime_wire * L_um;
  end function;

  function R_wire_ohm(L_um : real) return real is
  begin
    return Rprime_wire * L_um;
  end function;

  function C_gate_F(W_um : real) return real is
  begin
    return Cgate_per_um_F * W_um;
  end function;

  function tpd_elmore_s_rc(Rw, Cw, R_drv_ohm : real; C_extra_F : real := 0.0) return real is
  begin
    return 0.69 * R_drv_ohm * (Cw + Cint_stage_F + C_extra_F) + 0.38 * Rw * Cw; -- seconds
  end function;

  function tpd_elmore_s(L_um, R_drv_ohm : real; C_extra_F : real := 0.0) return real is
    constant Cw : real := C_wire_F(L_um);
    constant Rw : real := R_wire_ohm(L_um);
  begin
    return tpd_elmore_s_rc(Rw, Cw, R_drv_ohm, C_extra_F);
  end function;

  function tpd_elmore(L_um, R_drv_ohm : real; C_extra_F : real := 0.0) return real is
  begin
    return tpd_elmore_s(L_um, R_drv_ohm, C_extra_F) * 1.0e9;
  end function;

  function tpd_elmore_rc(Rw, Cw, R_drv_ohm : real; C_extra_F : real := 0.0) return real is
  begin
    return tpd_elmore_s_rc(Rw, Cw, R_drv_ohm, C_extra_F) * 1.0e9;
  end function;

  function tpd_z(T_plh : real := 0.0; T_phl : real := 0.0; T_min : real := T_Z_min) return real is
    variable T_pd : real;
  begin
    if T_plh >= T_phl then T_pd := T_plh; else T_pd := T_phl; end if;
    if 0.3 * T_pd > T_min then return 0.3 * T_pd; else return T_min; end if;
  end function;
end package body;
