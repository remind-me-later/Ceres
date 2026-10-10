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
    #[inline]
    pub fn new(code: &str) -> Result<Self, Error> {
        // Code consist of nine-digit hex numbers: "ABC-DEF-GHI"
        // AB, new data
        // FCDE, memory address, XORed by $F000
        // GI, old data, XORed by $BA and rotated left by two
        // H, Unknown, maybe checksum and/or else
        let code = code.trim().as_bytes();

        if code.len() != 11 {
            return Err(Error::InvalidGameGenieCodeLength { actual: code.len() });
        }

        for pos in [3, 7] {
            if code[usize::from(pos)] != b'-' {
                return Err(Error::InvalidGameGenieCodeExpectedHyphen { pos });
            }
        }

        let mut d = [0; 9];
        for (digit, pos) in d.iter_mut().zip([0, 1, 2, 4, 5, 6, 8, 9, 10]) {
            *digit = hex_digit(code[usize::from(pos)])
                .ok_or(Error::InvalidGameGenieCodeNotHexDigit { pos })?;
        }

        let ab = (d[0] << 4) | d[1];
        let cdef = d[2..6]
            .iter()
            .fold(0, |acc, &x| (acc << 4) | u16::from(x));
        let gh = (d[6] << 4) | d[7];
        let i = d[8];

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

const fn hex_digit(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

impl fmt::Display for GameGenieCode {
    #[inline]
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

#[cfg(test)]
mod tests {
    use super::GameGenieCode;
    use crate::Error;
    use alloc::string::ToString as _;

    #[test]
    fn round_trip() {
        let code = GameGenieCode::new(" 00A-17B-C49 ").map(|c| c.to_string());
        assert_eq!(code.ok().as_deref(), Some("00A-17B-C49"), "parse and print");
    }

    #[test]
    fn rejects_malformed_codes() {
        // Non-ASCII text and extra hyphens used to panic while slicing.
        assert!(
            matches!(
                GameGenieCode::new("a\u{e9}-cde-abc"),
                Err(Error::InvalidGameGenieCodeNotHexDigit { pos: 1 })
            ),
            "non-ASCII digit"
        );
        assert!(
            matches!(
                GameGenieCode::new("AB--DEF-GHI"),
                Err(Error::InvalidGameGenieCodeNotHexDigit { pos: 2 })
            ),
            "extra hyphen"
        );
        assert!(
            matches!(
                GameGenieCode::new("00A17B-C49"),
                Err(Error::InvalidGameGenieCodeLength { actual: 10 })
            ),
            "short code"
        );
        assert!(
            matches!(
                GameGenieCode::new("00A-17BxC49"),
                Err(Error::InvalidGameGenieCodeExpectedHyphen { pos: 7 })
            ),
            "missing hyphen"
        );
    }
}
