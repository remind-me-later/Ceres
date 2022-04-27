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
    action: Action,
    direction: Direction,
    use_directions: bool,
    use_actions: bool,
}

impl Joypad {
    pub const fn new() -> Self {
        Self {
            action: Action::empty(),
            direction: Direction::empty(),
            use_directions: true,
            use_actions: true,
        }
    }

    fn press_action_button(&mut self, interrupts: &mut Interrupts, action_button: Action) {
        self.action.insert(action_button);

        if self.use_actions {
            interrupts.request(Interrupt::JOYPAD);
        }
    }

    fn press_direction_button(&mut self, interrupts: &mut Interrupts, direction_button: Direction) {
        self.direction.insert(direction_button);

        if self.use_directions {
            interrupts.request(Interrupt::JOYPAD);
        }
    }

    pub fn press(&mut self, interrupts: &mut Interrupts, button: Button) {
        match button {
            Button::Left => self.press_direction_button(interrupts, Direction::LEFT),
            Button::Up => self.press_direction_button(interrupts, Direction::UP),
            Button::Right => self.press_direction_button(interrupts, Direction::RIGHT),
            Button::Down => self.press_direction_button(interrupts, Direction::DOWN),
            Button::A => self.press_action_button(interrupts, Action::A),
            Button::B => self.press_action_button(interrupts, Action::B),
            Button::Select => self.press_action_button(interrupts, Action::SELECT),
            Button::Start => self.press_action_button(interrupts, Action::START),
        }
    }

    pub fn release(&mut self, button: Button) {
        match button {
            Button::Left => self.direction.remove(Direction::LEFT),
            Button::Up => self.direction.remove(Direction::UP),
            Button::Right => self.direction.remove(Direction::RIGHT),
            Button::Down => self.direction.remove(Direction::DOWN),
            Button::A => self.action.remove(Action::A),
            Button::B => self.action.remove(Action::B),
            Button::Select => self.action.remove(Action::SELECT),
            Button::Start => self.action.remove(Action::START),
        }
    }

    pub const fn read(&self) -> u8 {
        let action_bits = if self.use_actions {
            self.action.bits() | (1 << 5)
        } else {
            0
        };

        let direction_bits = if self.use_directions {
            self.direction.bits() | (1 << 4)
        } else {
            0
        };

        // pressed on low
        !(action_bits | direction_bits)
    }

    pub fn write(&mut self, val: u8) {
        self.use_actions = (val >> 5) & 1 == 0;
        self.use_directions = (val >> 4) & 1 == 0;
    }
}
