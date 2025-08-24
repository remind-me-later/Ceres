use crate::Error;

pub struct GameGenie {
    codes: Vec<GameGenieCode>,
}

impl GameGenie {
    pub fn new<'a, I: IntoIterator<Item = &'a str>>(codes: I) -> Result<Self, Error> {
        let mut game_genie = Self { codes: Vec::new() };
        for code in codes {
            game_genie.codes.push(GameGenieCode::new(code)?);
        }

        game_genie.codes.sort_by_key(|code| code.address);

        // TODO: Fail on duplicates?
        game_genie.codes.dedup_by_key(|code| code.address);

        Ok(game_genie)
    }

    pub fn query(&self, address: u16, old_data: u8) -> Option<u8> {
        // Since the vector is sorted by address, we can use binary search
        let code = self
            .codes
            .binary_search_by_key(&address, |code| code.address)
            .map(|index| &self.codes[index])
            .ok()?;

        // Check if the old data matches
        (code.old_data == old_data).then_some(code.new_data)
    }
}

struct GameGenieCode {
    new_data: u8,
    old_data: u8,
    address: u16,
}

impl GameGenieCode {
    #[expect(clippy::string_slice)]
    fn new(code: &str) -> Result<Self, Error> {
        // Code consist of nine-digit hex numbers: "ABC-DEF-GHI"
        // AB, new data
        // FCDE, memory address, XORed by $F000
        // GI, old data, XORed by $BA and rotated left by two
        // H, Unknown, maybe checksum and/or else
        if code.len() != 9
            || !code.is_ascii()
            || code.chars().nth(3) != Some('-')
            || code.chars().nth(7) != Some('-')
        {
            return Err(Error::InvalidGameGenieCode {
                code: code.to_owned(),
            });
        }

        let ab = u8::from_str_radix(&code[0..2], 16).map_err(|_| Error::InvalidGameGenieCode {
            code: code.to_owned(),
        })?;

        let cdef =
            u16::from_str_radix(&code[4..8], 16).map_err(|_| Error::InvalidGameGenieCode {
                code: code.to_owned(),
            })?;

        let ghi =
            u8::from_str_radix(&code[8..10], 16).map_err(|_| Error::InvalidGameGenieCode {
                code: code.to_owned(),
            })?;

        let f = (cdef >> 12) & 0x0F;
        let cde = (cdef >> 4) & 0x0FFF;
        let g = (ghi >> 4) & 0x0F;
        let i = ghi & 0x0F;

        let new_data = ab;
        let address = ((f << 12) | cde) ^ 0xF000;
        let old_data = (((g << 4) | i) ^ 0xBA).rotate_left(2);

        Ok(Self {
            new_data,
            old_data,
            address,
        })
    }
}
