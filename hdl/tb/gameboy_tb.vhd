-- SPDX-License-Identifier: CC-BY-SA-4.0

-- A Game Boy board around the DMG-CPU B: the crystal, the reset, the video
-- RAM, the work RAM, a cartridge, the other end of the serial link, and
-- what the testbench observes: the pins (pin_trace) and the result of a
-- test ROM.
--
-- It follows dmg-sim's testbench (dmg_cpu_b_gameboy.sv; Copyright Michael
-- Singer, CC-BY-SA 4.0), so that the two simulations can be compared pin
-- by pin: the crystal input takes the value of the crystal output every
-- 119 ns, and the reset is released after 128 of these steps.
--
-- Generics:
--   SIM_TIME     simulated time, as a string ("40 ms": a string, since GHDL
--                cannot set a time generic from the command line)
--   ROM_FILE     cartridge ROM, "" for no cartridge (the boot ROM must be in
--                bootrom.bin in the working directory)
--   TRACE_FILE   where to write every change of the pins, "<time> <pin>
--                <value>"; "-" for none (nvc cannot pass an empty string)
--   TEST         how a test ROM reports its result, to stop the simulation
--                when it is known:
--                  "mooneye"      registers B to L at LD B,B
--                  "gbmicrotest"  FF80 to FF82 in high RAM
--                  "blargg"       "Passed" or "Failed" on the serial port
--                  ""             none: run for SIM_TIME
--   RESULT_FILE  where to write the result: "pass|fail|timeout <details>"
--                ("-" for none)
--   SERIAL_FILE  where to write what the serial port sends ("-" for none)

library ieee;
use ieee.std_logic_1164.all;
use work.tb_util.all;

entity gameboy_tb is
  generic (
    SIM_TIME    : string := "1 ms";
    ROM_FILE    : string := "";
    TRACE_FILE  : string := "trace.txt";
    TEST        : string := "";
    RESULT_FILE : string := "result.txt";
    SERIAL_FILE : string := "-"
  );
end entity;

architecture sim of gameboy_tb is
  constant T_half_step : delay_length := 119 ns;

  -- Crystal and reset
  signal xi, xo : std_logic := '0';
  signal nreset : std_logic := '0';

  -- Cartridge bus
  signal phi, nrd, nwr, ncs : std_logic;
  signal a : std_logic_vector(15 downto 0);
  signal d : std_logic_vector(7 downto 0);

  -- Video RAM bus
  signal nmrd, nmwr, nmcs : std_logic;
  signal ma : std_logic_vector(12 downto 0);
  signal md : std_logic_vector(7 downto 0);

  -- Serial link, buttons, LCD
  signal sout, sin, sck : std_logic;
  signal p10, p11, p12, p13, p14, p15 : std_logic;
  signal cpg, cp, cpl, fr, st, s, m1 : std_logic;
  signal ld : std_logic_vector(1 downto 0);

  -- Audio
  signal rout, lout : real;

  -- The CPU has read the entry point of the cartridge (0100): the boot ROM
  -- is done, and the state of the chip is meaningful to the tests.
  signal booted : boolean := false;
begin
  -- The chip. The parts below that look inside it find it by this name.
  dut : entity work.dmg_cpu_b
    port map (
      ck1_pad => xi, ck2_pad => xo, reset_n_pad => nreset, phi_pad => phi,
      rd_n_pad => nrd, wr_n_pad => nwr, cs_n_pad => ncs,
      a_pad0 => a(0), a_pad1 => a(1), a_pad2 => a(2), a_pad3 => a(3),
      a_pad4 => a(4), a_pad5 => a(5), a_pad6 => a(6), a_pad7 => a(7),
      a_pad8 => a(8), a_pad9 => a(9), a_pad10 => a(10), a_pad11 => a(11),
      a_pad12 => a(12), a_pad13 => a(13), a_pad14 => a(14), a_pad15 => a(15),
      d_pad0 => d(0), d_pad1 => d(1), d_pad2 => d(2), d_pad3 => d(3),
      d_pad4 => d(4), d_pad5 => d(5), d_pad6 => d(6), d_pad7 => d(7),
      vin_pad => 0.0,
      mrd_n_pad => nmrd, mwr_n_pad => nmwr, mcs_n_pad => nmcs,
      ma_pad0 => ma(0), ma_pad1 => ma(1), ma_pad2 => ma(2), ma_pad3 => ma(3),
      ma_pad4 => ma(4), ma_pad5 => ma(5), ma_pad6 => ma(6), ma_pad7 => ma(7),
      ma_pad8 => ma(8), ma_pad9 => ma(9), ma_pad10 => ma(10), ma_pad11 => ma(11),
      ma_pad12 => ma(12),
      md_pad0 => md(0), md_pad1 => md(1), md_pad2 => md(2), md_pad3 => md(3),
      md_pad4 => md(4), md_pad5 => md(5), md_pad6 => md(6), md_pad7 => md(7),
      sout_pad => sout, sin_pad => sin, sck_pad => sck,
      p10_pad => p10, p11_pad => p11, p12_pad => p12, p13_pad => p13,
      p14_pad => p14, p15_pad => p15,
      cpg_pad => cpg, cp_pad => cp, cpl_pad => cpl, fr_pad => fr, st_pad => st, s_pad => s,
      ld_pad0 => ld(0), ld_pad1 => ld(1), m1_pad => m1,
      so1_pad => rout, so2_pad => lout,
      t1_pad => '0', t2_pad => '0', nmi_n_pad => open
    );

  -- Bus keepers on the board, as in dmg-sim.
  keep_a  : for i in a'range generate  k : entity work.keeper port map (n => a(i));  end generate;
  keep_d  : for i in d'range generate  k : entity work.keeper port map (n => d(i));  end generate;
  keep_md : for i in md'range generate k : entity work.keeper port map (n => md(i)); end generate;
  keep_rd   : entity work.keeper port map (n => nrd);
  keep_wr   : entity work.keeper port map (n => nwr);
  keep_mrd  : entity work.keeper port map (n => nmrd);
  keep_mwr  : entity work.keeper port map (n => nmwr);
  keep_mcs  : entity work.keeper port map (n => nmcs);

  -- The 4 MiHz crystal on XI and XO.
  crystal : process is
  begin
    wait for T_half_step;
    xi <= xo;
  end process;

  reset : process is
  begin
    wait for 128 * T_half_step;
    nreset <= '1';
    wait;
  end process;

  vram : entity work.video_ram
    port map (ncs => nmcs, nwe => nmwr, noe => nmrd, a => ma, d => md);

  wram : entity work.work_ram
    port map (ncs => ncs, nrd => nrd, nwr => nwr, a => a, d => d);

  cart : entity work.cartridge
    generic map (ROM_FILE => ROM_FILE)
    port map (nreset => nreset, ncs => ncs, nrd => nrd, nwr => nwr, a => a, d => d);

  serial : entity work.serial_receiver
    generic map (SERIAL_FILE => SERIAL_FILE, BLARGG => TEST = "blargg", RESULT_FILE => RESULT_FILE)
    port map (sck => sck, sout => sout);

  -- The address settles after /RD falls: any time during the read counts.
  entry : process (nrd, a) is
  begin
    if to_x01(nrd) = '0' and to_x01(a) = x"0100" and not booted then
      report "the CPU reads the entry point 0100";
      booted <= true;
    end if;
  end process;

  oam : entity work.oam_conflict;

  mooneye : entity work.mooneye_result
    generic map (ENABLED => TEST = "mooneye", RESULT_FILE => RESULT_FILE)
    port map (booted => booted);

  gbmicrotest : entity work.gbmicrotest_result
    generic map (ENABLED => TEST = "gbmicrotest", RESULT_FILE => RESULT_FILE);

  trace : entity work.pin_trace
    generic map (TRACE_FILE => TRACE_FILE)
    port map (
      a => a, d => d, ma => ma, md => md, ld => ld,
      phi => phi, nrd => nrd, nwr => nwr, ncs => ncs, nmrd => nmrd, nmwr => nmwr, nmcs => nmcs,
      cpg => cpg, cp => cp, cpl => cpl, fr => fr, st => st, s => s, m1 => m1, sout => sout
    );

  stop : process is
  begin
    wait for time'value(SIM_TIME);
    if TEST = "" then
      finish(RESULT_FILE, "done");
    else
      finish(RESULT_FILE, "timeout");
    end if;
  end process;
end architecture;
