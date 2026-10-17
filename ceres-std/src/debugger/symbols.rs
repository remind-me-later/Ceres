//! RGBDS symbol files (`rgblink -n game.sym`).
//!
//! A symbol file has a line per label, `BB:AAAA Name`: the bank and the
//! address in hex, then the name. Banked memory (ROMX, VRAM, SRAM and WRAMX)
//! puts labels of different banks at the same addresses; `Symbols` tells
//! them apart by bank. Lines with no address (constants) are skipped.

use std::path::Path;

/// The bank a symbol is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bank {
    /// The boot ROM, while it's mapped.
    Boot,
    Number(u32),
}

impl Bank {
    /// Reads a bank as symbol files write it: `BOOT` or a number in hex.
    #[must_use]
    #[inline]
    pub fn parse(bank: &str) -> Option<Self> {
        if bank.eq_ignore_ascii_case("BOOT") {
            Some(Self::Boot)
        } else {
            u32::from_str_radix(bank, 16).ok().map(Self::Number)
        }
    }
}

impl core::fmt::Display for Bank {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Boot => f.write_str("BOOT"),
            Self::Number(bank) => write!(f, "{bank:02X}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    pub bank: Bank,
    pub addr: u16,
    pub name: String,
}

/// The areas of memory a symbol can't cross into.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Region {
    Rom0,
    Romx,
    Vram,
    Sram,
    Wram0,
    Wramx,
    /// Echo RAM, OAM, the I/O registers and HRAM.
    High,
}

impl Region {
    const fn of(addr: u16) -> Self {
        match addr {
            0x0000..=0x3FFF => Self::Rom0,
            0x4000..=0x7FFF => Self::Romx,
            0x8000..=0x9FFF => Self::Vram,
            0xA000..=0xBFFF => Self::Sram,
            0xC000..=0xCFFF => Self::Wram0,
            0xD000..=0xDFFF => Self::Wramx,
            0xE000..=0xFFFF => Self::High,
        }
    }

    const fn is_banked(self) -> bool {
        matches!(self, Self::Romx | Self::Vram | Self::Sram | Self::Wramx)
    }
}

pub struct Symbols {
    /// Sorted by address.
    symbols: Vec<Symbol>,
    /// Indices into `symbols`, sorted by name.
    by_name: Vec<usize>,
}

impl Symbols {
    /// Reads the symbol file next to the ROM at `rom_path` (`game.sym` for
    /// `game.gb`), if there is one.
    #[must_use]
    #[inline]
    pub fn for_rom(rom_path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(rom_path.with_extension("sym")).ok()?;
        let symbols = Self::parse(&text);
        (!symbols.symbols.is_empty()).then_some(symbols)
    }

    /// Reads a symbol file. Lines it doesn't understand are skipped.
    #[must_use]
    #[inline]
    pub fn parse(text: &str) -> Self {
        let mut symbols: Vec<Symbol> = text.lines().filter_map(parse_line).collect();
        symbols.sort_by_key(|symbol| symbol.addr);
        let mut by_name: Vec<usize> = (0..symbols.len()).collect();
        by_name.sort_by(|&a, &b| symbols[a].name.cmp(&symbols[b].name));
        Self { symbols, by_name }
    }

    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.symbols.len()
    }

    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }

    /// The banks with a label at `addr`.
    #[must_use]
    #[inline]
    pub fn banks_at(&self, addr: u16) -> Vec<Bank> {
        let mut banks: Vec<Bank> = self.at(addr).map(|symbol| symbol.bank).collect();
        banks.dedup();
        banks
    }

    /// Whether a label in `bank` at `addr` is what the CPU sees there when
    /// `mapped` is: the same bank, or for unbanked memory any cartridge
    /// bank (labels there are in bank 0).
    #[must_use]
    #[inline]
    pub fn is_mapped(addr: u16, bank: Bank, mapped: Bank) -> bool {
        match (bank, mapped) {
            (Bank::Boot, Bank::Boot) => true,
            (Bank::Number(_), Bank::Number(_)) => !Region::of(addr).is_banked() || bank == mapped,
            _ => false,
        }
    }

    /// The label `addr` is at or after, with how far after: a boot ROM
    /// label if `bank` is the boot ROM, else one of the cartridge's, in
    /// `bank` if `addr` is in banked memory.
    #[must_use]
    #[inline]
    pub fn label_for(&self, addr: u16, bank: Bank) -> Option<(&str, u16)> {
        let region = Region::of(addr);
        let end = self.symbols.partition_point(|symbol| symbol.addr <= addr);
        self.symbols[..end]
            .iter()
            .rev()
            .take_while(|symbol| Region::of(symbol.addr) == region)
            .find(|symbol| Self::is_mapped(addr, symbol.bank, bank))
            .map(|symbol| (symbol.name.as_str(), addr - symbol.addr))
    }

    /// The symbol called `name`.
    #[must_use]
    #[inline]
    pub fn find(&self, name: &str) -> Option<&Symbol> {
        let names = &self.by_name;
        let i = names.partition_point(|&i| self.symbols[i].name.as_str() < name);
        names
            .get(i)
            .map(|&i| &self.symbols[i])
            .filter(|symbol| symbol.name == name)
    }

    /// The names starting with `prefix`, in order (for completion).
    #[inline]
    pub fn complete<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a str> {
        let names = &self.by_name;
        let start = names.partition_point(|&i| self.symbols[i].name.as_str() < prefix);
        names[start..]
            .iter()
            .map(|&i| self.symbols[i].name.as_str())
            .take_while(move |name| name.starts_with(prefix))
    }

    fn at(&self, addr: u16) -> impl Iterator<Item = &Symbol> {
        let start = self.symbols.partition_point(|symbol| symbol.addr < addr);
        self.symbols[start..]
            .iter()
            .take_while(move |symbol| symbol.addr == addr)
    }
}

fn parse_line(line: &str) -> Option<Symbol> {
    let line = line.split(';').next().unwrap_or_default();
    let mut words = line.split_whitespace();
    let (bank, addr) = words.next()?.split_once(':')?;
    let name = words.next()?.to_owned();
    let bank = Bank::parse(bank)?;
    let addr = u16::from_str_radix(addr, 16).ok()?;
    Some(Symbol { bank, addr, name })
}

#[cfg(test)]
mod tests {
    use super::{Bank, Symbols};

    const SYM: &str = "\
; File generated by rgblink
00:0150 Main
00:0158 Main.loop
01:4000 BankOne
02:4000 BankTwo
02:4010 BankTwo.end
00:c000 wCounter
02:d000 wBanked
BOOT:0000 EntryPoint
not a symbol
";

    #[test]
    fn parse() {
        let symbols = Symbols::parse(SYM);
        assert_eq!(symbols.len(), 8);
        assert_eq!(symbols.symbols[0].bank, Bank::Boot);
        assert_eq!(symbols.symbols[1].name, "Main");
    }

    #[test]
    fn names() {
        let symbols = Symbols::parse(SYM);
        assert_eq!(
            symbols.find("BankTwo").map(|s| s.bank),
            Some(Bank::Number(2))
        );
        assert!(symbols.find("BankTw").is_none());
        assert_eq!(
            symbols.complete("BankT").collect::<Vec<_>>(),
            ["BankTwo", "BankTwo.end"]
        );
        assert_eq!(symbols.complete("Main.").collect::<Vec<_>>(), ["Main.loop"]);
    }

    #[test]
    fn banks() {
        let symbols = Symbols::parse(SYM);
        assert_eq!(symbols.banks_at(0x4000), [Bank::Number(1), Bank::Number(2)]);
        assert_eq!(symbols.banks_at(0x4001), []);
        assert_eq!(symbols.banks_at(0x0000), [Bank::Boot]);
        assert!(Symbols::is_mapped(0x4000, Bank::Number(2), Bank::Number(2)));
        assert!(!Symbols::is_mapped(
            0x4000,
            Bank::Number(2),
            Bank::Number(1)
        ));
        assert!(Symbols::is_mapped(0xC000, Bank::Number(0), Bank::Number(0)));
        assert!(!Symbols::is_mapped(0x0000, Bank::Number(0), Bank::Boot));

        let bank = Bank::Number;
        assert_eq!(symbols.label_for(0x015A, bank(0)), Some(("Main.loop", 2)));
        assert_eq!(symbols.label_for(0x4012, bank(1)), Some(("BankOne", 0x12)));
        assert_eq!(symbols.label_for(0x4012, bank(2)), Some(("BankTwo.end", 2)));
        assert_eq!(symbols.label_for(0x4012, bank(3)), None);
        assert_eq!(
            symbols.label_for(0x0010, bank(0)),
            None,
            "not the boot ROM's"
        );
        assert_eq!(
            symbols.label_for(0x0010, Bank::Boot),
            Some(("EntryPoint", 0x10))
        );
        // A label doesn't reach into the next region.
        assert_eq!(
            symbols.label_for(0x3FFF, bank(0)),
            Some(("Main.loop", 0x3EA7))
        );
        assert_eq!(symbols.label_for(0x8000, bank(0)), None);
        assert_eq!(symbols.label_for(0xD004, bank(2)), Some(("wBanked", 4)));
    }
}
