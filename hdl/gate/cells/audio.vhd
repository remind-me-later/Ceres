-- SPDX-License-Identifier: CC-BY-SA-4.0

-- Analog audio: the four DACs, the mixers and the volume amplifiers, by
-- behaviour (voltages in V).
--
-- Ported by hand from dmg-sim's dmg_cpu_b/cells/audio.sv (Copyright Michael
-- Singer, CC-BY-SA 4.0), which models it by behaviour.

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

entity dmg_audio is
  generic (
    L_rout : real := work.dmg_audio_param.L_rout; L_lout : real := work.dmg_audio_param.L_lout;
    W_gate_rout : real := 0.0; W_gate_lout : real := 0.0
  );
  port (
    ch1_0, ch1_1, ch1_2, ch1_3, ch2_0, ch2_1, ch2_2, ch2_3 : in std_logic;
    ch3_0, ch3_1, ch3_2, ch3_3, ch4_0, ch4_1, ch4_2, ch4_3 : in std_logic;
    ena, ena_n, ch1_ena_n, ch2_ena_n, ch3_ena_n, ch4_ena_n : in std_logic;
    r_vin_ena, rmixer0, rmixer1, rmixer2, rmixer3 : in std_logic;
    l_vin_ena, lmixer0, lmixer1, lmixer2, lmixer3 : in std_logic;
    rvolume_n0, rvolume_n1, rvolume_n2, lvolume_n0, lvolume_n1, lvolume_n2 : in std_logic;
    rout, lout : out real;
    vin : in real
  );
end entity;

architecture sim of dmg_audio is
  -- A quarter cycle of delay against glitches between the bits of a value.
  constant dl : delay_length := 60 ns;

  function num(v : std_logic_vector) return natural is
  begin
    if is_x(v) then return 0; else return to_integer(unsigned(to_x01(v))); end if;
  end function;

  signal ch1, ch2, ch3, ch4 : natural := 0;
  signal on_dl : boolean := false;
  signal rmix, lmix : std_logic_vector(1 to 5) := (others => '0');
  signal rvol, lvol : natural := 1;
begin
  ch1 <= num(ch1_3 & ch1_2 & ch1_1 & ch1_0) after dl;
  ch2 <= num(ch2_3 & ch2_2 & ch2_1 & ch2_0) after dl;
  ch3 <= num(ch3_3 & ch3_2 & ch3_1 & ch3_0) after dl;
  ch4 <= num(ch4_3 & ch4_2 & ch4_1 & ch4_0) after dl;
  on_dl <= (to_x01(ena) and not to_x01(ena_n)) = '1' after dl;
  rmix <= (rmixer3 and not ch4_ena_n and ena and not ena_n) &
          (rmixer2 and not ch3_ena_n and ena and not ena_n) &
          (rmixer1 and not ch2_ena_n and ena and not ena_n) &
          (rmixer0 and not ch1_ena_n and ena and not ena_n) &
          (r_vin_ena and ena and not ena_n) after dl;
  lmix <= (lmixer3 and not ch4_ena_n and ena and not ena_n) &
          (lmixer2 and not ch3_ena_n and ena and not ena_n) &
          (lmixer1 and not ch2_ena_n and ena and not ena_n) &
          (lmixer0 and not ch1_ena_n and ena and not ena_n) &
          (l_vin_ena and ena and not ena_n) after dl;
  rvol <= 7 - num(rvolume_n2 & rvolume_n1 & rvolume_n0) + 1 after dl;
  lvol <= 7 - num(lvolume_n2 & lvolume_n1 & lvolume_n0) + 1 after dl;

  process (ch1, ch2, ch3, ch4, on_dl, rmix, lmix, rvol, lvol, vin) is
    -- The DACs output 0.7 V to 3.7 V in 0.2 V steps.
    function dac(n : natural) return real is
    begin
      return real(n) / 5.0 + 0.7;
    end function;
    -- Mixing and volume invert and scale around 2.2 V; the mixer resistors
    -- are four times the maximum of the volume rheostat.
    function mix(m : std_logic_vector(1 to 5); c1, c2, c3, c4 : natural; v_in : real; vol : natural) return real is
      variable s : real := 0.0;
    begin
      if m(1) = '1' then s := s + (dac(c4) - 2.2) * 0.25; end if;
      if m(2) = '1' then s := s + (dac(c3) - 2.2) * 0.25; end if;
      if m(3) = '1' then s := s + (dac(c2) - 2.2) * 0.25; end if;
      if m(4) = '1' then s := s + (dac(c1) - 2.2) * 0.25; end if;
      if m(5) = '1' then s := s + (v_in - 2.2) * 0.25; end if;
      return s * (-(real(vol) / 8.0)) + 2.2;
    end function;
    -- The output is capped at 0.3 V and 4.3 V.
    function cap(v : real) return real is
    begin
      if v > 4.3 then return 4.3; elsif v < 0.3 then return 0.3; else return v; end if;
    end function;
  begin
    if on_dl then
      rout <= cap(mix(rmix, ch1, ch2, ch3, ch4, vin, rvol));
      lout <= cap(mix(lmix, ch1, ch2, ch3, ch4, vin, lvol));
    else
      rout <= 2.2;
      lout <= 2.2;
    end if;
  end process;
end architecture;
