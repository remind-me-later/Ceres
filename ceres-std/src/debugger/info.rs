//! What the debugger shows of the machine: registers, interrupts, the
//! display, the timer, the DMAs and the cartridge, and states saved or
//! loaded.

use {
    super::{
        mapped_bank,
        symbols::{Bank, Symbols},
    },
    ceres_core::{AudioCallback, Gb, HdmaKind, OamDmaPhase},
    std::time::{SystemTime, UNIX_EPOCH},
};

/// The interrupts by IF bit.
const INTERRUPTS: [&str; 5] = ["VBlank", "STAT", "timer", "serial", "joypad"];

pub(super) fn registers<A: AudioCallback>(gb: &Gb<A>) -> String {
    let af = gb.cpu_af();
    let flag = |bit: u16, name: char| if af & bit == 0 { '-' } else { name };
    format!(
        "AF = ${af:04X} (flags {}{}{}{})\n\
         BC = ${:04X}\n\
         DE = ${:04X}\n\
         HL = ${:04X}\n\
         SP = ${:04X}\n\
         PC = ${:04X}\n\
         IME = {}  HALT = {}",
        flag(0x80, 'Z'),
        flag(0x40, 'N'),
        flag(0x20, 'H'),
        flag(0x10, 'C'),
        gb.cpu_bc(),
        gb.cpu_de(),
        gb.cpu_hl(),
        gb.cpu_sp(),
        gb.cpu_pc(),
        u8::from(gb.cpu_ime()),
        u8::from(gb.cpu_is_halted()),
    )
}

pub(super) fn interrupts<A: AudioCallback>(gb: &Gb<A>) -> String {
    let flags = gb.peek_mem(0xFF0F);
    let enabled = gb.peek_mem(0xFFFF);
    let pending = flags & enabled & 0x1F;
    let names: Vec<&str> = INTERRUPTS
        .iter()
        .enumerate()
        .filter(|&(bit, _)| pending & (1 << bit) != 0)
        .map(|(_, &name)| name)
        .collect();
    format!(
        "IF = ${flags:02X}  IE = ${enabled:02X}  pending: {}\nIME = {}  HALT = {}",
        if names.is_empty() {
            "none".to_owned()
        } else {
            names.join(", ")
        },
        u8::from(gb.cpu_ime()),
        u8::from(gb.cpu_is_halted()),
    )
}

pub(super) fn lcd<A: AudioCallback>(gb: &Gb<A>) -> String {
    let ppu = gb.ppu_state();
    let registers = format!(
        "LCDC = ${:02X}  STAT = ${:02X}  LY = ${:02X}  LYC = ${:02X}",
        gb.peek_mem(0xFF40),
        gb.peek_mem(0xFF41),
        gb.peek_mem(0xFF44),
        gb.peek_mem(0xFF45),
    );
    if !ppu.lcd_on {
        return format!("LCD off\n{registers}");
    }
    format!(
        "mode {}, line {} dot {} (half dot {}), pixel pipeline on line {}{}\n{registers}",
        ppu.mode,
        ppu.line,
        ppu.half_dot / 2,
        ppu.half_dot,
        ppu.pipeline_line,
        if ppu.double_speed {
            ", double speed"
        } else {
            ""
        },
    )
}

pub(super) fn timer<A: AudioCallback>(gb: &Gb<A>) -> String {
    let timer = gb.timer_state();
    // T-cycles per TIMA increment, by TAC's clock select.
    let period = [1024, 16, 64, 256][usize::from(timer.tac & 3)];
    format!(
        "DIV counter = ${:04X} (DIV = ${:02X})\n\
         TIMA = ${:02X}  TMA = ${:02X}  TAC = ${:02X} ({}, every {period} T-cycles)\n\
         reload in {} T-cycles, interrupt in {} (0: none pending)",
        timer.div_counter,
        timer.div_counter >> 8,
        timer.tima,
        timer.tma,
        timer.tac,
        if timer.tac & 4 == 0 { "off" } else { "on" },
        timer.reload_pending,
        timer.irq_countdown,
    )
}

pub(super) fn dma<A: AudioCallback>(gb: &Gb<A>) -> String {
    let oam = gb.oam_dma_state();
    let oam = match oam.phase {
        OamDmaPhase::Idle => "idle".to_owned(),
        OamDmaPhase::StartUp => format!("starting, from ${:04X}", oam.source),
        OamDmaPhase::Copy { next } => {
            format!("copying OAM byte ${next:02X} from ${:04X}", oam.source)
        }
        _ => "last step".to_owned(),
    };
    let hdma = gb.hdma_state();
    let hdma = match hdma.kind {
        HdmaKind::Idle => "idle".to_owned(),
        kind => format!(
            "{} ${:04X} -> ${:04X}, {} blocks left",
            match kind {
                HdmaKind::HBlank {
                    requested: true, ..
                } => "HBlank (block requested)",
                HdmaKind::HBlank { .. } => "HBlank",
                _ => "general",
            },
            hdma.source,
            hdma.destination,
            hdma.blocks_left,
        ),
    };
    format!("OAM DMA: {oam}\nHDMA: {hdma}")
}

pub(super) fn cartridge<A: AudioCallback>(gb: &Gb<A>) -> String {
    let banks = gb.rom_banks();
    let cart = format!(
        "ROM0 bank {:02X}  ROMX bank {:02X}  SRAM bank {:02X}",
        banks.rom0,
        banks.romx,
        gb.ram_bank()
    );
    if gb.model().is_cgb_hardware() {
        format!(
            "{cart}\nWRAMX bank {:02X}  VRAM bank {:02X}",
            gb.wram_bank(),
            gb.vram_bank()
        )
    } else {
        cart
    }
}

/// Seconds since the Unix epoch, which dates the cartridge clock in states.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

pub(super) fn save<A: AudioCallback>(gb: &Gb<A>, path: &str) -> String {
    let mut state = Vec::new();
    gb.save_state(&mut state, now());
    match std::fs::write(path, &state) {
        Ok(()) => format!("saved {path} ({} bytes)", state.len()),
        Err(e) => format!("save: {path}: {e}"),
    }
}

/// Loads a state; `Ok` says it was.
pub(super) fn load<A: AudioCallback>(gb: &mut Gb<A>, path: &str) -> Result<String, String> {
    let state = std::fs::read(path).map_err(|e| format!("load: {path}: {e}"))?;
    // The state is checked before anything changes.
    gb.load_state(&state, now())
        .map_err(|e| format!("load: {path}: {e}"))?;
    Ok(format!("loaded {path}"))
}

/// `BANK:ADDR` as symbol files write it, with the label it's in.
pub(super) fn describe(symbols: Option<&Symbols>, bank: Bank, addr: u16) -> String {
    match symbols.and_then(|symbols| symbols.label_for(addr, bank)) {
        Some((label, 0)) => format!("{bank}:{addr:04X} {label}"),
        Some((label, offset)) => format!("{bank}:{addr:04X} {label}+{offset}"),
        None => format!("{bank}:{addr:04X}"),
    }
}

/// `addr` in the bank mapped there, with its label.
pub(super) fn describe_mapped<A: AudioCallback>(
    gb: &Gb<A>,
    symbols: Option<&Symbols>,
    addr: u16,
) -> String {
    describe(symbols, mapped_bank(gb, addr), addr)
}
