use bitflags::bitflags;

use crate::interrupts::{Interrupt, Interrupts};

bitflags! {
    struct Action: u8 {
        const A      = 1;
        const B      = 2;
        const SELECT = 4;
        const START  = 8;
    }
}

bitflags! {
    struct Direction: u8 {
        const RIGHT = 1;
        const LEFT  = 2;
        const UP    = 4;
        const DOWN  = 8;
    }
}

#[derive(Clone, Copy)]
pub enum Button {
    Left,
    Up,
    Right,
    Down,
    A,
    B,
    Select,
    Start,
}

pub struct Joypad {
    act: Action,
    dir: Direction,
    use_dir: bool,
    use_act: bool,
}

impl Joypad {
    pub const fn new() -> Self {
        Self {
            act: Action::empty(),
            dir: Direction::empty(),
            use_dir: false,
            use_act: false,
        }
    }

    fn press_action_button(&mut self, ints: &mut Interrupts, button: Action) {
        self.act.insert(button);

        if self.use_act {
            ints.request(Interrupt::JOYPAD);
        }
    }

    fn press_direction_button(&mut self, ints: &mut Interrupts, button: Direction) {
        self.dir.insert(button);

        if self.use_dir {
            ints.request(Interrupt::JOYPAD);
        }
    }

    pub fn press(&mut self, ints: &mut Interrupts, button: Button) {
        match button {
            Button::Left => self.press_direction_button(ints, Direction::LEFT),
            Button::Up => self.press_direction_button(ints, Direction::UP),
            Button::Right => self.press_direction_button(ints, Direction::RIGHT),
            Button::Down => self.press_direction_button(ints, Direction::DOWN),
            Button::A => self.press_action_button(ints, Action::A),
            Button::B => self.press_action_button(ints, Action::B),
            Button::Select => self.press_action_button(ints, Action::SELECT),
            Button::Start => self.press_action_button(ints, Action::START),
        }
    }

    pub fn release(&mut self, button: Button) {
        match button {
            Button::Left => self.dir.remove(Direction::LEFT),
            Button::Up => self.dir.remove(Direction::UP),
            Button::Right => self.dir.remove(Direction::RIGHT),
            Button::Down => self.dir.remove(Direction::DOWN),
            Button::A => self.act.remove(Action::A),
            Button::B => self.act.remove(Action::B),
            Button::Select => self.act.remove(Action::SELECT),
            Button::Start => self.act.remove(Action::START),
        }
    }

    pub const fn read(&self) -> u8 {
        let act_bits = if self.use_act {
            self.act.bits() | (1 << 5)
        } else {
            0
        };

        let dir_bits = if self.use_dir {
            self.dir.bits() | (1 << 4)
        } else {
            0
        };

        // pressed on low
        !(act_bits | dir_bits)
    }

    pub fn write(&mut self, val: u8) {
        self.use_act = (val >> 5) & 1 == 0;
        self.use_dir = (val >> 4) & 1 == 0;
    }
}
