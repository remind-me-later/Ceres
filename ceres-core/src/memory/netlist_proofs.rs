//! Kani proof that ceres's HRAM and boot ROM are the netlist's bus side, on
//! the DMG (`cargo kani -p ceres-core --harness netlist_bus_side`).
//!
//! `NetBus` (`ceres_netlist::bus`) transcribes `hdl/rtl/external_bus.vhd`,
//! `high_ram.vhd` and `boot_rom.vhd` (with the joypad and the serial port)
//! phase by phase. Ceres's M-cycle boundary is the start of the netlist's E,
//! as for the timer and IF: the CPU's access is ceres's at that boundary,
//! and on the netlist it writes in E to G and reads what the bus holds in H
//! (the core's read, `sm83::check`).
//!
//! The accesses are those the chip decodes itself, whose request does not
//! leave it (`NetChip::internal_access`): HRAM (FF80 to FFFE), FF50, the
//! boot ROM's 0000 to 00FF while it is mapped, and the addresses of FF00 to
//! FF7F that no block of the chip decodes (`memory::undecoded`). The
//! cartridge bus's pins, and so the board, play no part in them. Where nothing drives the data bus, it
//! keeps the precharge of phase A, FF.
//!
//! Which of ceres's blocks an address reaches (`Gb::read_mem`, `write_mem`)
//! is checked against the same map, for every address, by
//! `memory::tests::dmg_map_is_the_netlists`.

use {
    super::{Hram, undecoded},
    crate::bootrom::Bootrom,
    ceres_netlist::{
        Phase,
        bus::{BusInputs, Cartridge, Mbc, NetBoard, NetBus},
    },
};

/// A CPU access in the M-cycle.
#[derive(Clone, Copy)]
enum Access {
    None,
    Read(u16),
    Write(u16, u8),
}

/// The addresses of the proof: what the chip decodes itself, but for the
/// joypad and the serial port.
const fn bus_side(addr: u16, mapped: bool) -> bool {
    let hram = (addr >> 7 == 0x1FF) & (addr != 0xFFFF);
    hram | (addr == 0xFF50) | undecoded(addr) | (mapped & (addr >> 8 == 0))
}

fn any_access(mapped: bool) -> Access {
    let addr: u16 = kani::any();
    kani::assume(bus_side(addr, mapped));
    match kani::any::<u8>() % 3 {
        0 => Access::None,
        1 => Access::Read(addr),
        _ => Access::Write(addr, kani::any()),
    }
}

/// The bus side's inputs in `phase` of an M-cycle with `access`: a request
/// the chip keeps (`mreq` low), no DMA on the cartridge bus, no reset.
fn inputs(phase: Phase, access: Access) -> BusInputs {
    let (addr, read, write, data) = match access {
        Access::None => (0, false, false, 0xFF),
        Access::Read(addr) => (addr, true, false, 0xFF),
        Access::Write(addr, val) => (addr, false, true, val),
    };
    // Only E to H are the access's: from A, the next M-cycle's.
    let ours = matches!(phase, Phase::E | Phase::F | Phase::G | Phase::H);
    let (read, write) = (read & ours, write & ours);
    BusInputs {
        addr: if ours { addr } else { 0 },
        read,
        write,
        mreq: false,
        data,
        clk_t4: matches!(phase, Phase::G | Phase::H),
        exec_phase: !matches!(phase, Phase::A),
        cpu_wr_sync: write & matches!(phase, Phase::E | Phase::F | Phase::G),
        cpu_ext_cs_en: false,
        clk_1mhz: matches!(phase, Phase::A),
        starts_a: matches!(phase, Phase::A),
        reset2: false,
        reset: false,
        dma_addr_ext: false,
        dma_addr: 0,
        div: 0,
        p10_13: 0x0F,
        sin: true,
        sck: true,
        latch_glitch: None,
    }
}

/// A board with nothing on it: the accesses never select it.
fn empty_board() -> NetBoard {
    NetBoard {
        work_ram: alloc::vec::Vec::new(),
        cartridge: Cartridge {
            rom: alloc::vec::Vec::new(),
            ram: alloc::vec::Vec::new(),
            mbc: Mbc::None,
            has_ram: false,
            enabled: false,
            rom_bank: 1,
            ram_bank: 0,
            mode: false,
        },
    }
}

/// The netlist's M-cycle, from E to D, and the read as H has it: what the
/// bus side drives, or the precharge.
fn net_m_cycle(net: &mut NetBus, access: Access, boot_rom: &[u8]) -> Option<u8> {
    let mut board = empty_board();
    let mut read = None;
    let mut phase = Phase::E;
    for _ in 0..8 {
        let out = net.phase(&inputs(phase, access), &mut board, boot_rom);
        if matches!(phase, Phase::H) && matches!(access, Access::Read(_)) {
            read = Some(out.data_out.unwrap_or(0xFF));
        }
        phase = phase.next();
    }
    read
}

/// Ceres's M-cycle: the access, as `Gb::read_mem` and `write_mem` route
/// these addresses on the DMG.
fn ceres_m_cycle(hram: &mut Hram, boot: &mut Bootrom, access: Access) -> Option<u8> {
    match access {
        Access::None => None,
        Access::Read(addr) => Some(match addr {
            0x0000..=0x00FF => boot.read(addr).unwrap_or(0xFF),
            0xFF50 => boot.read_bank(),
            0xFF80..=0xFFFE => hram.read((addr & 0xFF) as u8),
            _ => 0xFF,
        }),
        Access::Write(addr, val) => {
            match addr {
                0xFF50 => boot.write_bank(val),
                0xFF80..=0xFFFE => hram.write((addr & 0xFF) as u8, val),
                _ => {}
            }
            None
        }
    }
}

/// From any HRAM and mapping, the same on both sides, and any boot ROM, an
/// M-cycle with any access to the bus side gives the same read and leaves
/// HRAM and the mapping the same.
// @assumes transcription-bus
// @assumes dmg-undecoded-io
// @assumes mapping-m-cycle-at-e
#[kani::proof]
// The comparisons of HRAM and the copy of the image are loops of 127 and
// 256 bytes.
#[kani::unwind(258)]
fn netlist_bus_side() {
    let image: [u8; 0x100] = kani::any();
    let mapped: bool = kani::any();

    let mut net = NetBus::power_on();
    net.high_ram = kani::any();
    net.unmapped = !mapped;

    let mut hram = Hram::default();
    *hram.hram_mut() = net.high_ram;
    let mut boot = Bootrom::from_image(alloc::borrow::Cow::Owned(image.to_vec()));
    if !mapped {
        boot.disable();
    }

    let access = any_access(mapped);
    let net_read = net_m_cycle(&mut net, access, &image);
    let ceres_read = ceres_m_cycle(&mut hram, &mut boot, access);

    assert!(net_read == ceres_read, "the read differs");
    assert!(net.high_ram == *hram.hram(), "HRAM differs");
    assert!(net.unmapped == !boot.is_enabled(), "the mapping differs");
    let unmaps = matches!(access, Access::Write(0xFF50, val) if val & 1 != 0);
    let reads_ff50 = matches!(access, Access::Read(0xFF50));
    kani::cover!(mapped & unmaps, "FF50 unmaps the boot ROM");
    kani::cover!(
        mapped & reads_ff50,
        "FF50 reads FE while the boot ROM is mapped"
    );
}
