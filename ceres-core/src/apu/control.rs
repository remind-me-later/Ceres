use super::channels::Channels;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TriggerReset {
    Reset,
    None,
}

pub struct Control {
    is_enabled: bool,
    nr51: u8,
    right_output_volume: u8,
    left_output_volume: u8,
    right_vin_on: bool,
    left_vin_on: bool,
}

impl Control {
    pub fn new() -> Self {
        Self {
            is_enabled: false,
            nr51: 0,
            right_output_volume: 0,
            left_output_volume: 0,
            right_vin_on: false,
            left_vin_on: false,
        }
    }

    pub fn reset(&mut self) {
        self.left_output_volume = 0;
        self.left_vin_on = false;
        self.right_output_volume = 0;
        self.right_vin_on = false;
        self.nr51 = 0;
    }

    pub fn output_volumes(&self) -> (u8, u8) {
        (self.left_output_volume, self.right_output_volume)
    }

    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    pub fn read_nr50(&self) -> u8 {
        self.right_output_volume
            | (u8::from(self.right_vin_on) << 3)
            | (self.left_output_volume << 4)
            | (u8::from(self.left_vin_on) << 7)
    }

    pub fn write_nr50(&mut self, val: u8) {
        self.right_output_volume = val & 7;
        self.right_vin_on = val & (1 << 3) != 0;
        self.left_output_volume = (val >> 4) & 7;
        self.left_vin_on = val & (1 << 7) != 0;
    }

    pub fn read_nr51(&self) -> u8 {
        self.nr51
    }

    pub fn write_nr51(&mut self, val: u8) {
        self.nr51 = val;
    }

    pub fn read_nr52(&self, channels: &Channels) -> u8 {
        const READ_MASK: u8 = 0x70;
        READ_MASK | (u8::from(self.is_enabled) << 7) | channels.read_enable_u4()
    }

    pub fn write_nr52(&mut self, val: u8) -> TriggerReset {
        self.is_enabled = val & (1 << 7) != 0;

        if self.is_enabled {
            TriggerReset::None
        } else {
            TriggerReset::Reset
        }
    }

    pub fn channel_enabled_terminal_iter(&self) -> ChannelEnabledTerminalIterator {
        ChannelEnabledTerminalIterator {
            channel_index: 0,
            nr51: self.nr51,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ChannelEnabledTerminalIterator {
    channel_index: u8,
    nr51: u8,
}

impl Iterator for ChannelEnabledTerminalIterator {
    type Item = (bool, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.channel_index < 4 {
            let right_bit = 1 << self.channel_index;
            let left_bit = 1 << (self.channel_index + 4);
            let (is_left_enabled, is_right_enabled) =
                (self.nr51 & left_bit != 0, self.nr51 & right_bit != 0);

            self.channel_index += 1;
            Some((is_left_enabled, is_right_enabled))
        } else {
            None
        }
    }
}
