-- A Game Boy board around the DMG-CPU B: the crystal, the reset, the video
-- RAM, the work RAM and a cartridge (no memory bank controller, MBC1 or
-- MBC5, with its RAM).
--
-- It follows dmg-sim's testbench (dmg_cpu_b_gameboy.sv), so that the two
-- simulations can be compared pin by pin: the crystal input takes the value
-- of the crystal output every 119 ns, and the reset is released after 128
-- of these steps.
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
--                  "mooneye"      registers B to L at LD B,B (3, 5, 8, 13,
--                                 21, 34 if it passed)
--                  "gbmicrotest"  FF82 in high RAM (01 passed, FF failed)
--                  "blargg"       "Passed" or "Failed" on the serial port
--                  ""             none: run for SIM_TIME
--   RESULT_FILE  where to write the result: "pass|fail|timeout <details>"
--                ("-" for none)
--   SERIAL_FILE  where to write what the serial port sends ("-" for none)

library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;
use std.textio.all;
use work.dmg_memory.all;

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

  ---------------------------------------------------------------------------
  -- Cartridge

  type rom_t is array (natural range <>) of natural range 0 to 255;

  impure function read_rom(name : string; n : positive) return rom_t is
    type bin_file is file of character;
    file f : bin_file;
    variable c : character;
    variable r : rom_t(0 to n - 1);
  begin
    file_open(f, name, read_mode);
    for i in r'range loop
      read(f, c);
      r(i) := character'pos(c);
    end loop;
    file_close(f);
    return r;
  end function;

  impure function load_rom(name : string) return rom_t is
    type bin_file is file of character;
    file f : bin_file;
    variable status : file_open_status;
    variable c : character;
    variable n : natural := 0;
  begin
    if name = "" then
      return (0 => 16#FF#);
    end if;
    file_open(status, f, name, read_mode);
    assert status = open_ok report "cannot open " & name severity failure;
    while not endfile(f) loop
      read(f, c);
      n := n + 1;
    end loop;
    file_close(f);
    return read_rom(name, n);
  end function;

  constant cart_rom : rom_t := load_rom(ROM_FILE);
  constant has_rom  : boolean := ROM_FILE /= "";

  type mbc_t is (none, mbc1, mbc5);

  function mbc_of(rom : rom_t) return mbc_t is
  begin
    if rom'length <= 16#147# then
      return none;
    end if;
    case rom(16#147#) is
      when 16#00# | 16#08# | 16#09# => return none;
      when 16#01# | 16#02# | 16#03# => return mbc1;
      when 16#19# to 16#1E#         => return mbc5;
      when others =>
        report "unsupported cartridge type " & integer'image(rom(16#147#)) severity failure;
        return none;
    end case;
  end function;

  function ram_size_of(rom : rom_t) return natural is
  begin
    if rom'length <= 16#149# then
      return 0;
    end if;
    case rom(16#149#) is
      when 0      => return 0;
      when 1      => return 2048;
      when 2      => return 8192;
      when 3      => return 32768;
      when 4      => return 131072;
      when 5      => return 65536;
      when others => return 0;
    end case;
  end function;

  constant mbc      : mbc_t   := mbc_of(cart_rom);
  constant ram_size : natural := ram_size_of(cart_rom);

  -- The CPU has read the entry point of the cartridge (0100): the boot ROM
  -- is done, and the state of the chip is meaningful to the tests.
  signal booted : boolean := false;

  -- Memory bank controller registers
  signal mbc_ena      : boolean := false;
  signal mbc_rom_bank : natural range 0 to 511 := 0;
  signal mbc_ram_bank : natural range 0 to 15 := 0;
  signal mbc1_mode    : std_ulogic := '0';

  ---------------------------------------------------------------------------
  -- Traces and results

  -- "" or "-" for no file.
  function is_none(name : string) return boolean is
  begin
    return name = "" or name = "-";
  end function;

  impure function file_name(name : string) return string is
  begin
    if is_none(name) then return "/dev/null"; else return name; end if;
  end function;

  file trace  : text open write_mode is file_name(TRACE_FILE);
  file serial : text open write_mode is file_name(SERIAL_FILE);

  procedure log(name : string; v : std_ulogic) is
    variable l : line;
    variable c : character;
  begin
    if is_none(TRACE_FILE) then
      return;
    end if;
    case v is
      when '0' | 'L' => c := '0';
      when '1' | 'H' => c := '1';
      when 'Z'       => c := 'z';
      when others    => c := 'x';
    end case;
    write(l, to_string(now, ps) & " " & name & " " & c);
    writeline(trace, l);
  end procedure;

  -- Ends the simulation with a result.
  procedure finish(result : string) is
    file f : text;
    variable l : line;
  begin
    report "result: " & result;
    if not is_none(RESULT_FILE) then
      file_open(f, RESULT_FILE, write_mode);
      write(l, result);
      writeline(f, l);
      file_close(f);
    end if;
    std.env.finish;
  end procedure;

  function hex(v : std_logic_vector) return string is
  begin
    if is_x(v) then return "xx"; else return to_hstring(to_x01(v)); end if;
  end function;

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

  function index(v : std_logic_vector) return natural is
  begin
    return to_integer(unsigned(to_x01(v)));
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
        mem(index(ma)) := x"A5";
      else
        mem(index(ma)) := to_x01(md);
      end if;
    end if;
    if to_x01(nmcs) = '0' and to_x01(nmrd) = '0' and to_x01(nmwr) = '1' then
      if is_x(ma) then
        md <= (others => 'X');
      else
        md <= mem(index(ma));
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
        mem(index(a(12 downto 0))) := x"5A";
      else
        mem(index(a(12 downto 0))) := to_x01(d);
      end if;
    end if;
    if to_x01(ncs) = '0' and to_x01(a(14)) = '1' and to_x01(nrd) = '0' and to_x01(nwr) = '1' then
      if is_x(a(12 downto 0)) then
        d <= (others => 'X');
      else
        d <= mem(index(a(12 downto 0)));
      end if;
    else
      d <= (others => 'Z');
    end if;
  end process;

  -- The registers of the memory bank controller (dmg-sim's mbc1.sv and
  -- mbc5.sv), written on the rising edge of /WR, cleared by the reset.
  mbc_regs : process (nreset, nwr) is
    variable dv : natural;
  begin
    if to_x01(nreset) = '0' then
      mbc_ena      <= false;
      mbc_rom_bank <= 0;
      mbc_ram_bank <= 0;
      mbc1_mode    <= '0';
    elsif rising_edge(nwr) and not is_x(a(15 downto 12)) and not is_x(d) then
      dv := index(d);
      case mbc is
        when mbc1 =>
          case index(a(15 downto 13)) is
            when 0      => mbc_ena <= dv mod 16 = 16#A#;
            when 1      => mbc_rom_bank <= dv mod 32;
            when 2      => mbc_ram_bank <= dv mod 4;
            when 3      => mbc1_mode <= d(0);
            when others => null;
          end case;
        when mbc5 =>
          case index(a(15 downto 12)) is
            when 0      => mbc_ena <= dv mod 16 = 16#A#;
            when 2      => mbc_rom_bank <= (mbc_rom_bank / 256) * 256 + dv;
            when 3      => mbc_rom_bank <= (dv mod 2) * 256 + mbc_rom_bank mod 256;
            when 4      => mbc_ram_bank <= dv mod 16;
            when others => null;
          end case;
        when none => null;
      end case;
    end if;
  end process;

  -- Cartridge ROM and RAM on the bus.
  cartridge : process (a, nrd, nwr, ncs, nreset, mbc_ena, mbc_rom_bank, mbc_ram_bank, mbc1_mode) is
    variable ram      : bytes(0 to 131071) := (others => (others => '0'));
    variable rom_cs   : boolean;
    variable ram_cs   : boolean;
    variable rom_bank : natural;
    variable ram_bank : natural;
    variable adr      : natural;
    variable known    : boolean;
  begin
    known := not is_x(a);
    rom_bank := 0;
    ram_bank := 0;
    case mbc is
      when none =>
        rom_cs := to_x01(a(15)) = '0';
        ram_cs := to_x01(ncs) = '0' and to_x01(a(13)) = '1';
      when mbc1 =>
        rom_cs := to_x01(nreset) = '0' or (to_x01(a(15)) = '0' and to_x01(nrd) = '0');
        ram_cs := mbc_ena and to_x01(ncs) = '0' and to_x01(a(14)) = '0';
        if to_x01(nreset) = '1' and to_x01(a(14)) = '1' then
          rom_bank := mbc_rom_bank;
          if rom_bank = 0 then rom_bank := 1; end if;
        end if;
        if mbc1_mode = '1' or to_x01(a(14)) = '1' then ram_bank := mbc_ram_bank; end if;
      when mbc5 =>
        rom_cs := to_x01(a(15)) = '0';
        ram_cs := mbc_ena and to_x01(ncs) = '0' and to_x01(a(14)) = '0';
        if to_x01(nreset) = '1' and to_x01(a(14)) = '1' then rom_bank := mbc_rom_bank; end if;
        if to_x01(a(14)) = '1' then ram_bank := mbc_ram_bank; end if;
    end case;
    ram_cs := ram_cs and ram_size /= 0;

    -- RAM written on the rising edge of /WR
    if nwr'event and to_x01(nwr) = '1' and ram_cs and known then
      adr := (ram_bank * 8192 + index(a(12 downto 0))) mod ram_size;
      if is_x(d) then ram(adr) := x"5A"; else ram(adr) := to_x01(d); end if;
    end if;

    if has_rom and rom_cs and to_x01(nrd) = '0' then
      if known then
        adr := (rom_bank * 16384 + index(a(13 downto 0))) mod cart_rom'length;
        if mbc = none then
          adr := index(a(14 downto 0)) mod cart_rom'length;
        end if;
        d <= std_logic_vector(to_unsigned(cart_rom(adr), 8));
      else
        d <= (others => 'X');
      end if;
    elsif ram_cs and to_x01(nrd) = '0' and to_x01(nwr) = '1' then
      if known then
        d <= ram((ram_bank * 8192 + index(a(12 downto 0))) mod ram_size);
      else
        d <= (others => 'X');
      end if;
    else
      d <= (others => 'Z');
    end if;
  end process;

  -- The address settles after /RD falls: any time during the read counts.
  entry : process (nrd, a) is
  begin
    if to_x01(nrd) = '0' and to_x01(a) = x"0100" and not booted then
      report "the CPU reads the entry point 0100";
      booted <= true;
    end if;
  end process;

  ---------------------------------------------------------------------------
  -- Serial port: what the Game Boy sends with its internal clock, a byte
  -- with the most significant bit first, sampled on the rising edge of SCK.

  serial_rx : process is
    variable byte  : std_logic_vector(7 downto 0);
    variable received : string(1 to 4096);
    variable len   : natural := 0;
    variable l     : line;
    variable ch    : character;
  begin
    loop
      for i in 7 downto 0 loop
        wait until rising_edge(sck);
        byte(i) := to_x01(sout);
      end loop;
      if is_x(byte) then
        ch := '?';
      else
        ch := character'val(index(byte));
      end if;
      if not is_none(SERIAL_FILE) then
        if ch = LF then
          writeline(serial, l);
        else
          write(l, ch);
        end if;
      end if;
      if len < received'length then
        len := len + 1;
        received(len) := ch;
      end if;
      if TEST = "blargg" and len >= 6 then
        if received(len - 5 to len) = "Passed" then
          finish("pass serial: " & received(1 to len));
        elsif received(len - 5 to len) = "Failed" then
          finish("fail serial: " & received(1 to len));
        end if;
      end if;
    end loop;
  end process;

  ---------------------------------------------------------------------------
  -- Observers of the chip's state, by external names (after the chip, which
  -- must be elaborated first). The names are relative, so that another
  -- design can instantiate this testbench.

  observe : block is
    alias hram is << signal ^.dut.u_high_ram.sram.mem_view : bytes(0 to 127) >>;
  begin
    -- A conflict between OAM DMA and the reading of OAM: dmg-sim's testbench
    -- (solve_oam_conflict) forces the data lines of both OAM halves, 20 ns
    -- after it starts and until it ends, to values computed once (Icarus
    -- evaluates its forces once). OAM half B, bit 3, has an alternative
    -- result on some devices, which dmg-sim leaves off.
    oam_conflict : block is
      alias oam_oe       is << signal ^.^.dut.n_oam_oe : std_ulogic >>;
      alias dma_addr_ext is << signal ^.^.dut.n_dma_addr_ext : std_ulogic >>;
      alias ralo is << signal ^.^.dut.n_ralo : std_ulogic >>;
      alias tune is << signal ^.^.dut.n_tune : std_ulogic >>;
      alias sera is << signal ^.^.dut.n_sera : std_ulogic >>;
      alias tenu is << signal ^.^.dut.n_tenu : std_ulogic >>;
      alias sysa is << signal ^.^.dut.n_sysa : std_ulogic >>;
      alias sugy is << signal ^.^.dut.n_sugy : std_ulogic >>;
      alias tube is << signal ^.^.dut.n_tube : std_ulogic >>;
      alias syzo is << signal ^.^.dut.n_syzo : std_ulogic >>;
      alias dout_a is << signal ^.^.dut.u_oam_a.dout : std_logic_vector(7 downto 0) >>;
      alias dout_b is << signal ^.^.dut.u_oam_b.dout : std_logic_vector(7 downto 0) >>;
      alias a_d0 is << signal ^.^.dut.n_oam_a_d0_n : std_logic >>;
      alias a_d1 is << signal ^.^.dut.n_oam_a_d1_n : std_logic >>;
      alias a_d2 is << signal ^.^.dut.n_oam_a_d2_n : std_logic >>;
      alias a_d3 is << signal ^.^.dut.n_oam_a_d3_n : std_logic >>;
      alias a_d4 is << signal ^.^.dut.n_oam_a_d4_n : std_logic >>;
      alias a_d5 is << signal ^.^.dut.n_oam_a_d5_n : std_logic >>;
      alias a_d6 is << signal ^.^.dut.n_oam_a_d6_n : std_logic >>;
      alias a_d7 is << signal ^.^.dut.n_oam_a_d7_n : std_logic >>;
      alias b_d0 is << signal ^.^.dut.n_oam_b_d0_n : std_logic >>;
      alias b_d1 is << signal ^.^.dut.n_oam_b_d1_n : std_logic >>;
      alias b_d2 is << signal ^.^.dut.n_oam_b_d2_n : std_logic >>;
      alias b_d3 is << signal ^.^.dut.n_oam_b_d3_n : std_logic >>;
      alias b_d4 is << signal ^.^.dut.n_oam_b_d4_n : std_logic >>;
      alias b_d5 is << signal ^.^.dut.n_oam_b_d5_n : std_logic >>;
      alias b_d6 is << signal ^.^.dut.n_oam_b_d6_n : std_logic >>;
      alias b_d7 is << signal ^.^.dut.n_oam_b_d7_n : std_logic >>;
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
    end block;

    -- gbmicrotest: FF80 is the result, FF81 the expected value, FF82 01 if
    -- the test passed and FF if it failed. High RAM stores its bytes with the
    -- bits of the data bus reversed.
    gbmicrotest : if TEST = "gbmicrotest" generate
      process is
        impure function byte_at(adr : natural) return std_logic_vector is
          variable v : std_logic_vector(7 downto 0);
        begin
          for i in 0 to 7 loop
            v(i) := hram(adr)(7 - i);
          end loop;
          return v;
        end function;
      begin
        wait on hram;
        if byte_at(2) = x"01" or byte_at(2) = x"FF" then
          -- RAM is written as long as /WR is low, and the bus holds the
          -- precharged FF before the CPU drives the byte: read it again
          -- once the write is over.
          wait for 2 us;
        end if;
        if byte_at(2) = x"01" or byte_at(2) = x"FF" then
          if byte_at(2) = x"01" then
            finish("pass FF80=" & hex(byte_at(0)) & " FF81=" & hex(byte_at(1)));
          else
            finish("fail FF80=" & hex(byte_at(0)) & " FF81=" & hex(byte_at(1)));
          end if;
        end if;
      end process;
    end generate;

    -- Mooneye: the test ends with LD B,B, with B, C, D, E, H, L = 3, 5, 8,
    -- 13, 21, 34 if it passed.
    mooneye : if TEST = "mooneye" generate
      alias op0 is << signal ^.^.dut.u_cpu.n_opcode0 : std_ulogic >>;
      alias op1 is << signal ^.^.dut.u_cpu.n_opcode1 : std_ulogic >>;
      alias op2 is << signal ^.^.dut.u_cpu.n_opcode2 : std_ulogic >>;
      alias op3 is << signal ^.^.dut.u_cpu.n_opcode3 : std_ulogic >>;
      alias op4 is << signal ^.^.dut.u_cpu.n_opcode4 : std_ulogic >>;
      alias op5 is << signal ^.^.dut.u_cpu.n_opcode5 : std_ulogic >>;
      alias op6 is << signal ^.^.dut.u_cpu.n_opcode6 : std_ulogic >>;
      alias op7 is << signal ^.^.dut.u_cpu.n_opcode7 : std_ulogic >>;
      alias b0 is << signal ^.^.dut.u_cpu.n_reg_b0 : std_ulogic >>;
      alias b1 is << signal ^.^.dut.u_cpu.n_reg_b1 : std_ulogic >>;
      alias b2 is << signal ^.^.dut.u_cpu.n_reg_b2 : std_ulogic >>;
      alias b3 is << signal ^.^.dut.u_cpu.n_reg_b3 : std_ulogic >>;
      alias b4 is << signal ^.^.dut.u_cpu.n_reg_b4 : std_ulogic >>;
      alias b5 is << signal ^.^.dut.u_cpu.n_reg_b5 : std_ulogic >>;
      alias b6 is << signal ^.^.dut.u_cpu.n_reg_b6 : std_ulogic >>;
      alias b7 is << signal ^.^.dut.u_cpu.n_reg_b7 : std_ulogic >>;
      alias c0 is << signal ^.^.dut.u_cpu.n_reg_c0 : std_ulogic >>;
      alias c1 is << signal ^.^.dut.u_cpu.n_reg_c1 : std_ulogic >>;
      alias c2 is << signal ^.^.dut.u_cpu.n_reg_c2 : std_ulogic >>;
      alias c3 is << signal ^.^.dut.u_cpu.n_reg_c3 : std_ulogic >>;
      alias c4 is << signal ^.^.dut.u_cpu.n_reg_c4 : std_ulogic >>;
      alias c5 is << signal ^.^.dut.u_cpu.n_reg_c5 : std_ulogic >>;
      alias c6 is << signal ^.^.dut.u_cpu.n_reg_c6 : std_ulogic >>;
      alias c7 is << signal ^.^.dut.u_cpu.n_reg_c7 : std_ulogic >>;
      alias d0 is << signal ^.^.dut.u_cpu.n_reg_d0 : std_ulogic >>;
      alias d1 is << signal ^.^.dut.u_cpu.n_reg_d1 : std_ulogic >>;
      alias d2 is << signal ^.^.dut.u_cpu.n_reg_d2 : std_ulogic >>;
      alias d3 is << signal ^.^.dut.u_cpu.n_reg_d3 : std_ulogic >>;
      alias d4 is << signal ^.^.dut.u_cpu.n_reg_d4 : std_ulogic >>;
      alias d5 is << signal ^.^.dut.u_cpu.n_reg_d5 : std_ulogic >>;
      alias d6 is << signal ^.^.dut.u_cpu.n_reg_d6 : std_ulogic >>;
      alias d7 is << signal ^.^.dut.u_cpu.n_reg_d7 : std_ulogic >>;
      alias e0 is << signal ^.^.dut.u_cpu.n_reg_e0 : std_ulogic >>;
      alias e1 is << signal ^.^.dut.u_cpu.n_reg_e1 : std_ulogic >>;
      alias e2 is << signal ^.^.dut.u_cpu.n_reg_e2 : std_ulogic >>;
      alias e3 is << signal ^.^.dut.u_cpu.n_reg_e3 : std_ulogic >>;
      alias e4 is << signal ^.^.dut.u_cpu.n_reg_e4 : std_ulogic >>;
      alias e5 is << signal ^.^.dut.u_cpu.n_reg_e5 : std_ulogic >>;
      alias e6 is << signal ^.^.dut.u_cpu.n_reg_e6 : std_ulogic >>;
      alias e7 is << signal ^.^.dut.u_cpu.n_reg_e7 : std_ulogic >>;
      alias h0 is << signal ^.^.dut.u_cpu.n_reg_h0 : std_ulogic >>;
      alias h1 is << signal ^.^.dut.u_cpu.n_reg_h1 : std_ulogic >>;
      alias h2 is << signal ^.^.dut.u_cpu.n_reg_h2 : std_ulogic >>;
      alias h3 is << signal ^.^.dut.u_cpu.n_reg_h3 : std_ulogic >>;
      alias h4 is << signal ^.^.dut.u_cpu.n_reg_h4 : std_ulogic >>;
      alias h5 is << signal ^.^.dut.u_cpu.n_reg_h5 : std_ulogic >>;
      alias h6 is << signal ^.^.dut.u_cpu.n_reg_h6 : std_ulogic >>;
      alias h7 is << signal ^.^.dut.u_cpu.n_reg_h7 : std_ulogic >>;
      alias l0 is << signal ^.^.dut.u_cpu.n_reg_l0 : std_ulogic >>;
      alias l1 is << signal ^.^.dut.u_cpu.n_reg_l1 : std_ulogic >>;
      alias l2 is << signal ^.^.dut.u_cpu.n_reg_l2 : std_ulogic >>;
      alias l3 is << signal ^.^.dut.u_cpu.n_reg_l3 : std_ulogic >>;
      alias l4 is << signal ^.^.dut.u_cpu.n_reg_l4 : std_ulogic >>;
      alias l5 is << signal ^.^.dut.u_cpu.n_reg_l5 : std_ulogic >>;
      alias l6 is << signal ^.^.dut.u_cpu.n_reg_l6 : std_ulogic >>;
      alias l7 is << signal ^.^.dut.u_cpu.n_reg_l7 : std_ulogic >>;
    begin
      process is
        impure function opcode return std_logic_vector is
        begin
          return op7 & op6 & op5 & op4 & op3 & op2 & op1 & op0;
        end function;
        variable rb, rc, rd, re, rh, rl : std_logic_vector(7 downto 0);
      begin
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
            finish("pass");
          else
            finish("fail B=" & hex(rb) & " C=" & hex(rc) & " D=" & hex(rd) & " E=" & hex(re) &
                   " H=" & hex(rh) & " L=" & hex(rl));
          end if;
        end if;
      end process;
    end generate;
  end block;

  ---------------------------------------------------------------------------
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
    if TEST = "" then
      finish("done");
    else
      finish("timeout");
    end if;
  end process;
end architecture;
