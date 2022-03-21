use super::{
    frequency_data::FrequencyData,
    generic_channel::{GenericChannel, TriggerEvent},
};

pub const MAX_WAVE_CHANNEL_LENGTH: u16 = 0x100;

#[derive(Clone, Copy)]
enum Volume {
    Mute,
    Full,
    Half,
    Quarter,
}

impl Default for Volume {
    fn default() -> Self {
        Self::Mute
    }
}

impl Volume {
    pub const fn right_shift_value(self) -> u16 {
        match self {
            Volume::Mute => 4,
            Volume::Full => 0,
            Volume::Half => 1,
            Volume::Quarter => 2,
        }
    }
}

impl From<u8> for Volume {
    fn from(val: u8) -> Self {
        use self::Volume::{Full, Half, Mute, Quarter};
        match (val >> 5) & 3 {
            1 => Full,
            2 => Half,
            3 => Quarter,
            _ => Mute,
        }
    }
}

impl From<Volume> for u8 {
    fn from(volume: Volume) -> Self {
        use self::Volume::{Full, Half, Mute, Quarter};

        const READ_MASK: u8 = 0x9f;

        let bits = match volume {
            Mute => 0,
            Full => 1,
            Half => 2,
            Quarter => 3,
        };

        READ_MASK | (bits << 5)
    }
}

#[derive(Clone, Copy)]
pub enum Register {
    NR30,
    NR31,
    NR32,
    NR33,
    NR34,
}

const WAVE_RAM_SIZE: u8 = 0x10;
const WAVE_SAMPLE_SIZE: u8 = WAVE_RAM_SIZE * 2;
const WAVE_CHANNEL_PERIOD_MULTIPLIER: u16 = 2;

pub struct WaveChannel {
    generic_channel: GenericChannel<MAX_WAVE_CHANNEL_LENGTH>,
    frequency_data: FrequencyData<WAVE_CHANNEL_PERIOD_MULTIPLIER>,
    current_frequency_period: u16,
    sample_buffer: u8,
    wave_ram: [u8; WAVE_RAM_SIZE as usize],
    wave_samples: [u8; WAVE_SAMPLE_SIZE as usize], // we don't have u4 yet
    wave_sample_index: u8,
    volume: Volume,
    nr30: u8,
}

impl WaveChannel {
    pub const fn new() -> Self {
        let frequency_data: FrequencyData<WAVE_CHANNEL_PERIOD_MULTIPLIER> = FrequencyData::new();
        let current_frequency_period = frequency_data.period();

        Self {
            wave_ram: [0; WAVE_RAM_SIZE as usize],
            volume: Volume::Mute,
            generic_channel: GenericChannel::new(),
            frequency_data,
            current_frequency_period,
            sample_buffer: 0,
            wave_samples: [0; WAVE_SAMPLE_SIZE as usize],
            wave_sample_index: 0,
            nr30: 0,
        }
    }

    pub fn reset(&mut self) {
        self.generic_channel.reset();
        self.frequency_data.reset();
        self.volume = Volume::default();
        self.wave_sample_index = 0;
        self.current_frequency_period = 0;
        self.sample_buffer = 0;
        self.nr30 = 0;
    }

    pub const fn read_wave_ram(&self, address: u8) -> u8 {
        let index = (address - 0x30) as usize;
        self.wave_ram[index]
    }

    pub fn write_wave_ram(&mut self, address: u8, value: u8) {
        let index = (address - 0x30) as usize;
        self.wave_ram[index] = value;
        // upper 4 bits first
        self.wave_samples[index * 2] = value >> 4;
        self.wave_samples[index * 2 + 1] = value & 0xf;
    }

    pub fn read(&self, register: Register) -> u8 {
        const NR30_MASK: u8 = 0x7f;

        match register {
            Register::NR30 => NR30_MASK | self.nr30,
            Register::NR32 => self.volume.into(),
            Register::NR34 => self.generic_channel.read(),
            Register::NR31 | Register::NR33 => 0xff,
        }
    }

    pub fn write(&mut self, register: Register, val: u8) {
        match register {
            Register::NR30 => {
                self.nr30 = val;
                if val & (1 << 7) == 0 {
                    self.generic_channel.disable_dac();
                } else {
                    self.generic_channel.enable_dac();
                }
            }
            Register::NR31 => self.generic_channel.write_sound_length(val),
            Register::NR32 => self.volume = val.into(),
            Register::NR33 => self.frequency_data.set_low_byte(val),
            Register::NR34 => {
                self.frequency_data.set_high_byte(val);
                if self.generic_channel.write_control(val) == TriggerEvent::Trigger {
                    self.trigger();
                }
            }
        }
    }

    pub fn trigger(&mut self) {
        self.current_frequency_period = self.frequency_data.period();
        self.wave_sample_index = 0;
    }

    pub fn step_sample(&mut self) {
        if !self.is_enabled() {
            return;
        }

        if self.current_frequency_period == 0 {
            self.current_frequency_period = self.frequency_data.period();
            self.wave_sample_index = (self.wave_sample_index + 1) % WAVE_SAMPLE_SIZE;
            self.sample_buffer = self.wave_samples[self.wave_sample_index as usize];
        }

        self.current_frequency_period -= 1;
    }

    pub const fn output_volume(&self) -> u8 {
        self.sample_buffer >> self.volume.right_shift_value()
    }

    pub const fn is_enabled(&self) -> bool {
        self.generic_channel.is_enabled()
    }

    pub fn step_length(&mut self) {
        self.generic_channel.step_length();
    }

    pub fn mut_generic_channel(&mut self) -> &mut GenericChannel<MAX_WAVE_CHANNEL_LENGTH> {
        &mut self.generic_channel
    }
}
