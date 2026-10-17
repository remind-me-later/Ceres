-- A Game Boy board around the DMG-CPU B: the crystal, the reset, the video
-- RAM, the work RAM and a cartridge without a memory bank controller.
--
-- It follows dmg-sim's testbench (dmg_cpu_b_gameboy.sv), so that the two
-- simulations can be compared pin by pin: the crystal input takes the value
-- of the crystal output every 119 ns, and the reset is released after 128
-- of these steps.
--
-- Generics:
--   SIM_TIME    simulated time, as a string ("40 ms": a string, since GHDL
--               cannot set a time generic from the command line)
--   ROM_FILE    cartridge ROM, "" for no cartridge (the boot ROM must be in
--               bootrom.bin in the working directory)
--   TRACE_FILE  where to write every change of the pins: "<time> <pin> <value>"

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use std.textio.all;

entity gameboy_tb is
  generic (
    SIM_TIME   : string := "1 ms";
    ROM_FILE   : string := "";
    TRACE_FILE : string := "trace.txt"
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

  type bytes is array (natural range <>) of std_logic_vector(7 downto 0);

  impure function load_rom(name : string) return bytes is
    type bin_file is file of character;
    file f : bin_file;
    variable status : file_open_status;
    variable c : character;
    variable rom : bytes(0 to 32767) := (others => (others => '1'));
  begin
    if name = "" then
      return rom;
    end if;
    file_open(status, f, name, read_mode);
    assert status = open_ok report "cannot open " & name severity failure;
    for i in rom'range loop
      exit when endfile(f);
      read(f, c);
      rom(i) := std_logic_vector(to_unsigned(character'pos(c), 8));
    end loop;
    file_close(f);
    return rom;
  end function;

  constant cart_rom : bytes(0 to 32767) := load_rom(ROM_FILE);
  constant has_rom  : boolean := ROM_FILE /= "";

  file trace : text open write_mode is TRACE_FILE;

  procedure log(name : string; v : std_ulogic) is
    variable l : line;
    variable c : character;
  begin
    case v is
      when '0' | 'L' => c := '0';
      when '1' | 'H' => c := '1';
      when 'Z'       => c := 'z';
      when others    => c := 'x';
    end case;
    write(l, to_string(now, ps) & " " & name & " " & c);
    writeline(trace, l);
  end procedure;

  -- Pseudo-random initial content of the RAMs (dmg-sim uses $random).
  impure function noise(n : natural) return bytes is
    variable r : bytes(0 to n - 1);
    variable x : unsigned(31 downto 0) := x"2545F491";
  begin
    for i in r'range loop
      x := x xor shift_left(x, 13);
      x := x xor shift_right(x, 17);
      x := x xor shift_left(x, 5);
      r(i) := std_logic_vector(x(7 downto 0));
    end loop;
    return r;
  end function;
begin
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

  -- Video RAM (LH5164LN-10): /WE has precedence over /OE, and a write is
  -- continuous while /CS and /WE are low.
  vram : process (nmcs, nmwr, nmrd, ma, md) is
    variable mem : bytes(0 to 8191) := noise(8192);
  begin
    if to_x01(nmcs) = '0' and to_x01(nmwr) = '0' and not is_x(ma) then
      if is_x(md) then
        mem(to_integer(unsigned(to_x01(ma)))) := x"A5";
      else
        mem(to_integer(unsigned(to_x01(ma)))) := to_x01(md);
      end if;
    end if;
    if to_x01(nmcs) = '0' and to_x01(nmrd) = '0' and to_x01(nmwr) = '1' then
      if is_x(ma) then
        md <= (others => 'X');
      else
        md <= mem(to_integer(unsigned(to_x01(ma))));
      end if;
    else
      md <= (others => 'Z');
    end if;
  end process;

  -- Work RAM (C000-DFFF), written on the rising edge of /WR.
  wram : process (ncs, nrd, nwr, a) is
    variable mem : bytes(0 to 8191) := noise(8192);
  begin
    if rising_edge(nwr) and to_x01(ncs) = '0' and to_x01(a(14)) = '1' and not is_x(a(12 downto 0)) then
      if is_x(d) then
        mem(to_integer(unsigned(to_x01(a(12 downto 0))))) := x"5A";
      else
        mem(to_integer(unsigned(to_x01(a(12 downto 0))))) := to_x01(d);
      end if;
    end if;
    if to_x01(ncs) = '0' and to_x01(a(14)) = '1' and to_x01(nrd) = '0' and to_x01(nwr) = '1' then
      if is_x(a(12 downto 0)) then
        d <= (others => 'X');
      else
        d <= mem(to_integer(unsigned(to_x01(a(12 downto 0)))));
      end if;
    else
      d <= (others => 'Z');
    end if;
  end process;

  -- Cartridge ROM without a memory bank controller (0000-7FFF).
  cartridge : process (a, nrd) is
  begin
    if has_rom and to_x01(a(15)) = '0' and to_x01(nrd) = '0' then
      if is_x(a(14 downto 0)) then
        d <= (others => 'X');
      else
        d <= cart_rom(to_integer(unsigned(to_x01(a(14 downto 0)))));
      end if;
    else
      d <= (others => 'Z');
    end if;
  end process;

  -- Trace of the pins.
  trace_a  : for i in a'range generate  process (a(i))  is begin log("a_pad" & integer'image(i), a(i));   end process; end generate;
  trace_d  : for i in d'range generate  process (d(i))  is begin log("d_pad" & integer'image(i), d(i));   end process; end generate;
  trace_ma : for i in ma'range generate process (ma(i)) is begin log("ma_pad" & integer'image(i), ma(i)); end process; end generate;
  trace_md : for i in md'range generate process (md(i)) is begin log("md_pad" & integer'image(i), md(i)); end process; end generate;
  trace_ld : for i in ld'range generate process (ld(i)) is begin log("ld_pad" & integer'image(i), ld(i)); end process; end generate;
  process (phi)  is begin log("phi_pad", phi);   end process;
  process (nrd)  is begin log("rd_n_pad", nrd);  end process;
  process (nwr)  is begin log("wr_n_pad", nwr);  end process;
  process (ncs)  is begin log("cs_n_pad", ncs);  end process;
  process (nmrd) is begin log("mrd_n_pad", nmrd); end process;
  process (nmwr) is begin log("mwr_n_pad", nmwr); end process;
  process (nmcs) is begin log("mcs_n_pad", nmcs); end process;
  process (cpg)  is begin log("cpg_pad", cpg);   end process;
  process (cp)   is begin log("cp_pad", cp);     end process;
  process (cpl)  is begin log("cpl_pad", cpl);   end process;
  process (fr)   is begin log("fr_pad", fr);     end process;
  process (st)   is begin log("st_pad", st);     end process;
  process (s)    is begin log("s_pad", s);       end process;
  process (m1)   is begin log("m1_pad", m1);     end process;
  process (sout) is begin log("sout_pad", sout); end process;

  stop : process is
  begin
    wait for time'value(SIM_TIME);
    std.env.finish;
  end process;
end architecture;
