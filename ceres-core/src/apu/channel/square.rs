use super::{
    envelope::Envelope,
    frequency_data::FrequencyData,
    generic_channel::{GenericChannel, TriggerEvent},
};

const SQUARE_CHANNEL_PERIOD_MULTIPLIER: u16 = 4;
pub const MAX_SQUARE_CHANNEL_LENGTH: u16 = 64;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Period {
    SweepOff,
    Div1,
    Div2,
    Div3,
    Div4,
    Div5,
    Div6,
    Div7,
}

impl From<Period> for u8 {
    fn from(time: Period) -> Self {
        match time {
            // Apparently GB treats this internally as 8
            Period::SweepOff => 8,
            Period::Div1 => 1,
            Period::Div2 => 2,
            Period::Div3 => 3,
            Period::Div4 => 4,
            Period::Div5 => 5,
            Period::Div6 => 6,
            Period::Div7 => 7,
        }
    }
}

impl Default for Period {
    fn default() -> Self {
        Period::SweepOff
    }
}

impl From<u8> for Period {
    fn from(val: u8) -> Self {
        use self::Period::{Div1, Div2, Div3, Div4, Div5, Div6, Div7, SweepOff};
        // bits 6-4
        match (val >> 4) & 0x07 {
            1 => Div1,
            2 => Div2,
            3 => Div3,
            4 => Div4,
            5 => Div5,
            6 => Div6,
            7 => Div7,
            _ => SweepOff,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    High,
    Low,
}

impl Default for Direction {
    fn default() -> Self {
        Direction::High
    }
}

impl From<u8> for Direction {
    fn from(value: u8) -> Self {
        use Direction::{High, Low};
        match value & (1 << 3) {
            0 => High,
            _ => Low,
        }
    }
}

impl From<Direction> for u8 {
    fn from(dir: Direction) -> Self {
        use Direction::{High, Low};
        match dir {
            High => 0,
            Low => 1,
        }
    }
}

pub struct Sweep {
    period: Period,
    direction: Direction,
    shift_number: u8,
    timer: u8,
    shadow_frequency: u16,
    is_enabled: bool,
}

impl Sweep {
    fn new() -> Self {
        Self {
            period: Period::default(),
            direction: Direction::default(),
            shift_number: 0,
            timer: 0,
            shadow_frequency: 0,
            is_enabled: false,
        }
    }

    fn trigger(&mut self, generic_square_channel: &mut Common) {
        self.shadow_frequency = generic_square_channel.frequency().get();
        self.timer = self.period.into();
        self.is_enabled = self.period != Period::SweepOff && self.shift_number != 0;

        if self.shift_number != 0 {
            self.sweep_calculation(generic_square_channel);
        }
    }

    fn reset(&mut self) {
        self.period = Period::default();
        self.direction = Direction::default();
        self.shift_number = 0;
        self.is_enabled = false;
    }

    fn sweep_calculation(&mut self, generic_square_channel: &mut Common) {
        let tmp = self.shadow_frequency >> self.shift_number;
        self.shadow_frequency = if self.direction == Direction::High {
            self.shadow_frequency + tmp
        } else {
            self.shadow_frequency - tmp
        };

        if self.shadow_frequency > 0x7ff {
            generic_square_channel.set_enabled(false);
        } else if self.shift_number != 0 {
            generic_square_channel.set_frequency_data(self.shadow_frequency);
        }
    }

    fn step(&mut self, generic_square_channel: &mut Common) {
        self.timer -= 1;
        if self.timer == 0 {
            self.timer = self.period.into();
            if self.is_enabled {
                self.sweep_calculation(generic_square_channel);
            }
        }
    }

    fn read(&self) -> u8 {
        const MASK: u8 = 0x80;
        MASK | ((u8::from(self.period)) << 4)
            | (u8::from(self.direction) << 3)
            | (self.shift_number)
    }

    fn write(&mut self, value: u8) {
        self.period = value.into();
        self.direction = value.into();
        self.shift_number = value & 7;
    }
}

struct Common {
    frequency: FrequencyData<SQUARE_CHANNEL_PERIOD_MULTIPLIER>,
    duty_cycle: WaveDuty,
    duty_cycle_bit: u8,
    generic_channel: GenericChannel<MAX_SQUARE_CHANNEL_LENGTH>,
    current_frequency_period: u16,
    last_output: u8,
    envelope: Envelope,
}

impl Common {
    fn new() -> Self {
        let frequency = FrequencyData::new();
        let current_frequency_period = frequency.period();

        Self {
            frequency,
            current_frequency_period,
            duty_cycle: WaveDuty::Half,
            duty_cycle_bit: 0,
            generic_channel: GenericChannel::new(),
            last_output: 0,
            envelope: Envelope::new(),
        }
    }

    fn control_byte(&self) -> u8 {
        const MASK: u8 = 0x3f;
        MASK | u8::from(self.duty_cycle)
    }

    fn set_control_byte(&mut self, value: u8) {
        self.duty_cycle = value.into();
        self.generic_channel.write_sound_length(value);
    }

    fn set_low_byte(&mut self, value: u8) {
        self.frequency.set_low_byte(value);
    }

    fn high_byte(&self) -> u8 {
        self.generic_channel.read()
    }

    fn set_high_byte(&mut self, value: u8) -> TriggerEvent {
        self.frequency.set_high_byte(value);
        self.generic_channel.write_control(value)
    }

    fn trigger(&mut self) {
        self.current_frequency_period = self.frequency.period();
        self.last_output = 0;
        self.duty_cycle_bit = 0;
        self.envelope.trigger();
    }

    fn reset(&mut self) {
        self.generic_channel.reset();
        self.frequency.reset();
        self.duty_cycle = WaveDuty::HalfQuarter;
        self.duty_cycle_bit = 0;
        self.envelope.reset();
    }

    pub fn next_duty_cycle_value(&mut self) -> u8 {
        let output =
            (self.duty_cycle.duty_byte() & (1 << self.duty_cycle_bit)) >> self.duty_cycle_bit;
        self.duty_cycle_bit = (self.duty_cycle_bit + 1) % 8;
        output
    }

    fn frequency(&self) -> &FrequencyData<SQUARE_CHANNEL_PERIOD_MULTIPLIER> {
        &self.frequency
    }

    fn set_frequency_data(&mut self, frequency: u16) {
        self.frequency.set(frequency);
    }

    fn set_enabled(&mut self, val: bool) {
        self.generic_channel.set_enabled(val);
    }

    fn mut_generic_channel(&mut self) -> &mut GenericChannel<MAX_SQUARE_CHANNEL_LENGTH> {
        &mut self.generic_channel
    }

    fn step_sample(&mut self) {
        if !self.generic_channel.is_enabled() {
            return;
        }

        let (new_frequency_period, overflow) = self.current_frequency_period.overflowing_sub(1);

        if overflow {
            self.current_frequency_period = self.frequency.period();

            if self.generic_channel.is_enabled() {
                self.last_output = self.next_duty_cycle_value();
            }
        } else {
            self.current_frequency_period = new_frequency_period;
        }
    }

    fn output_volume(&self) -> u8 {
        self.last_output * self.envelope.volume()
    }

    fn step_envelope(&mut self) {
        self.envelope.step(&mut self.generic_channel);
    }

    fn read_envelope(&self) -> u8 {
        self.envelope.read()
    }

    fn write_envelope(&mut self, value: u8) {
        self.envelope.write(value, &mut self.generic_channel);
    }

    fn step_length(&mut self) {
        self.generic_channel.step_length();
    }

    fn is_enabled(&self) -> bool {
        self.generic_channel.is_enabled()
    }
}

#[derive(Clone, Copy)]
enum WaveDuty {
    HalfQuarter,
    Quarter,
    Half,
    ThreeQuarters,
}

impl WaveDuty {
    fn duty_byte(self) -> u8 {
        use self::WaveDuty::{Half, HalfQuarter, Quarter, ThreeQuarters};

        match self {
            HalfQuarter => 0b0000_0001,
            Quarter => 0b1000_0001,
            Half => 0b1000_0111,
            ThreeQuarters => 0b0111_1110,
        }
    }
}

impl From<u8> for WaveDuty {
    fn from(val: u8) -> Self {
        use self::WaveDuty::{Half, HalfQuarter, Quarter, ThreeQuarters};
        // bits 7-6
        match (val >> 6) & 3 {
            0 => HalfQuarter,
            1 => Quarter,
            2 => Half,
            _ => ThreeQuarters,
        }
    }
}

impl From<WaveDuty> for u8 {
    fn from(val: WaveDuty) -> Self {
        use self::WaveDuty::{Half, HalfQuarter, Quarter, ThreeQuarters};

        const WAVEDUTY_MASK: u8 = 0x3f;

        let byte = match val {
            HalfQuarter => 0,
            Quarter => 1,
            Half => 2,
            ThreeQuarters => 3,
        };
        // bits 7-6
        (byte << 6) | WAVEDUTY_MASK
    }
}

pub struct Chan1 {
    sweep: Sweep,
    common: Common,
}

impl Chan1 {
    pub fn new() -> Self {
        Self {
            sweep: Sweep::new(),
            common: Common::new(),
        }
    }

    pub fn reset(&mut self) {
        self.common.reset();
        self.sweep.reset();
    }

    pub fn read_nr10(&self) -> u8 {
        self.sweep.read()
    }

    pub fn read_nr11(&self) -> u8 {
        self.common.control_byte()
    }

    pub fn read_nr12(&self) -> u8 {
        self.common.read_envelope()
    }

    pub fn read_nr14(&self) -> u8 {
        self.common.high_byte()
    }

    pub fn write_nr10(&mut self, val: u8) {
        self.sweep.write(val);
    }

    pub fn write_nr11(&mut self, val: u8) {
        self.common.set_control_byte(val);
    }

    pub fn write_nr12(&mut self, val: u8) {
        self.common.write_envelope(val);
    }

    pub fn write_nr13(&mut self, val: u8) {
        self.common.set_low_byte(val);
    }

    pub fn write_nr14(&mut self, val: u8) {
        if self.common.set_high_byte(val) == TriggerEvent::Trigger {
            self.trigger();
        }
    }

    pub fn step_sample(&mut self) {
        self.common.step_sample();
    }

    pub fn step_sweep(&mut self) {
        if self.is_enabled() {
            self.sweep.step(&mut self.common);
        }
    }

    pub fn step_envelope(&mut self) {
        self.common.step_envelope();
    }

    pub fn trigger(&mut self) {
        self.common.trigger();
        self.sweep.trigger(&mut self.common);
    }

    pub fn output_volume(&self) -> u8 {
        self.common.output_volume()
    }

    pub fn is_enabled(&self) -> bool {
        self.common.is_enabled()
    }

    pub fn step_length(&mut self) {
        self.common.step_length();
    }

    pub fn mut_generic_channel(&mut self) -> &mut GenericChannel<MAX_SQUARE_CHANNEL_LENGTH> {
        self.common.mut_generic_channel()
    }
}

pub struct Chan2 {
    common: Common,
}

impl Chan2 {
    pub fn new() -> Self {
        Self {
            common: Common::new(),
        }
    }

    pub fn reset(&mut self) {
        self.common.reset();
    }

    pub fn read_nr21(&self) -> u8 {
        self.common.control_byte()
    }

    pub fn read_nr22(&self) -> u8 {
        self.common.read_envelope()
    }

    pub fn read_nr24(&self) -> u8 {
        self.common.high_byte()
    }

    pub fn write_nr21(&mut self, val: u8) {
        self.common.set_control_byte(val);
    }

    pub fn write_nr22(&mut self, val: u8) {
        self.common.write_envelope(val);
    }

    pub fn write_nr23(&mut self, val: u8) {
        self.common.set_low_byte(val);
    }

    pub fn write_nr24(&mut self, val: u8) {
        if self.common.set_high_byte(val) == TriggerEvent::Trigger {
            self.trigger();
        }
    }

    pub fn step_sample(&mut self) {
        self.common.step_sample();
    }

    pub fn step_envelope(&mut self) {
        self.common.step_envelope();
    }

    pub fn trigger(&mut self) {
        self.common.trigger();
    }

    pub fn output_volume(&self) -> u8 {
        self.common.output_volume()
    }

    pub fn is_enabled(&self) -> bool {
        self.common.is_enabled()
    }

    pub fn step_length(&mut self) {
        self.common.step_length();
    }

    pub fn mut_generic_channel(&mut self) -> &mut GenericChannel<MAX_SQUARE_CHANNEL_LENGTH> {
        self.common.mut_generic_channel()
    }
}
