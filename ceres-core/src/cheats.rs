use crate::Error;
use core::fmt;
use core::mem;

#[derive(Default)]
pub struct GameGenie {
    codes: [GameGenieCode; 3],
    number_of_active_codes: u8,
}

impl GameGenie {
    pub const fn activate_code(&mut self, code: GameGenieCode) -> Result<(), Error> {
        if self.number_of_active_codes < 3 {
            self.codes[self.number_of_active_codes as usize] = code;
            self.number_of_active_codes += 1;
            Ok(())
        } else {
            Err(Error::TooManyGameGenieCodes)
        }
    }

    pub fn active_codes(&self) -> &[GameGenieCode] {
        &self.codes[..self.number_of_active_codes as usize]
    }

    pub fn deactivate_code(&mut self, code: &GameGenieCode) {
        if let Some(pos) = self.codes[..self.number_of_active_codes as usize]
            .iter()
            .position(|c| c == code)
        {
            for i in pos..(self.number_of_active_codes as usize - 1) {
                self.codes[i] = mem::take(&mut self.codes[i + 1]);
            }

            self.number_of_active_codes -= 1;
        }
    }

    pub fn query(&self, address: u16, old_data: u8) -> Option<u8> {
        for code in &self.codes[..self.number_of_active_codes as usize] {
            if code.address == address && code.old_data == old_data {
                return Some(code.new_data);
            }
        }

        None
    }
}

#[derive(Default, PartialEq, Eq, Clone)]
pub struct GameGenieCode {
    address: u16,
    maybe_checksum: u8,
    new_data: u8,
    old_data: u8,
}

impl GameGenieCode {
    /// Creates a new `GameGenieCode` from a string.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidGameGenieCode` if the input string is not a valid Game Genie code.
    #[expect(clippy::string_slice)]
    pub fn new(code: &str) -> Result<Self, Error> {
        // Code consist of nine-digit hex numbers: "ABC-DEF-GHI"
        // AB, new data
        // FCDE, memory address, XORed by $F000
        // GI, old data, XORed by $BA and rotated left by two
        // H, Unknown, maybe checksum and/or else
        let code = code.trim();

        if code.len() != 11 {
            return Err(Error::InvalidGameGenieCodeLength { actual: code.len() });
        }

        if code.chars().nth(3) != Some('-') {
            return Err(Error::InvalidGameGenieCodeExpectedHyphen { pos: 3 });
        }

        if code.chars().nth(7) != Some('-') {
            return Err(Error::InvalidGameGenieCodeExpectedHyphen { pos: 7 });
        }

        let new_string = code.replace('-', "");

        let ab = u8::from_str_radix(&new_string[0..2], 16)
            .map_err(|_err| Error::InvalidGameGenieCodeNotHexDigit { pos: 0 })?;

        let cdef = u16::from_str_radix(&new_string[2..6], 16)
            .map_err(|_err| Error::InvalidGameGenieCodeNotHexDigit { pos: 2 })?;

        let gh = u8::from_str_radix(&new_string[6..8], 16)
            .map_err(|_err| Error::InvalidGameGenieCodeNotHexDigit { pos: 6 })?;

        let i = u8::from_str_radix(&new_string[8..9], 16)
            .map_err(|_err| Error::InvalidGameGenieCodeNotHexDigit { pos: 8 })?;

        let fcde = cdef.rotate_right(4);
        let gi = (gh & 0xF0) | i;

        let new_data = ab;
        let address = fcde ^ 0xF000;
        let old_data = gi.rotate_right(2) ^ 0xBA;
        let maybe_checksum = gh & 0x0F;

        Ok(Self {
            address,
            maybe_checksum,
            new_data,
            old_data,
        })
    }
}

impl fmt::Display for GameGenieCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let cdef = (self.address ^ 0xF000).rotate_left(4);
        let abc = (u16::from(self.new_data) << 4) | (cdef >> 12);
        let def = cdef & 0x0FFF;
        let gi = (self.old_data ^ 0xBA).rotate_left(2);
        let ghi = (u16::from(gi & 0xF0) << 4)
            | (u16::from(self.maybe_checksum) << 4)
            | u16::from(gi & 0x0F);

        write!(f, "{abc:03X}-{def:03X}-{ghi:03X}")
    }
}
