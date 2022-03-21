use bitflags::bitflags;

use crate::interrupts::{Interrupt, InterruptController};

bitflags! {
    struct ActionButtons: u8 {
        const A      = 1;
        const B      = 2;
        const SELECT = 4;
        const START  = 8;
    }
}

bitflags! {
    struct DirectionButtons: u8 {
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
    action_buttons: ActionButtons,
    direction_buttons: DirectionButtons,
    directions_enabled: bool,
    actions_enabled: bool,
}

impl Joypad {
    pub const fn new() -> Self {
        Self {
            action_buttons: ActionButtons::empty(),
            direction_buttons: DirectionButtons::empty(),
            directions_enabled: true,
            actions_enabled: true,
        }
    }

    fn press_action_button(
        &mut self,
        interrupts: &mut InterruptController,
        action_button: ActionButtons,
    ) {
        self.action_buttons.insert(action_button);

        if self.actions_enabled {
            interrupts.request(Interrupt::TIMER);
        }
    }

    fn press_direction_button(
        &mut self,
        interrupts: &mut InterruptController,
        direction_button: DirectionButtons,
    ) {
        self.direction_buttons.insert(direction_button);

        if self.directions_enabled {
            interrupts.request(Interrupt::TIMER);
        }
    }

    pub fn press(&mut self, interrupts: &mut InterruptController, button: Button) {
        match button {
            Button::Left => self.press_direction_button(interrupts, DirectionButtons::LEFT),
            Button::Up => self.press_direction_button(interrupts, DirectionButtons::UP),
            Button::Right => self.press_direction_button(interrupts, DirectionButtons::RIGHT),
            Button::Down => self.press_direction_button(interrupts, DirectionButtons::DOWN),
            Button::A => self.press_action_button(interrupts, ActionButtons::A),
            Button::B => self.press_action_button(interrupts, ActionButtons::B),
            Button::Select => self.press_action_button(interrupts, ActionButtons::SELECT),
            Button::Start => self.press_action_button(interrupts, ActionButtons::START),
        }
    }

    pub fn release(&mut self, button: Button) {
        match button {
            Button::Left => self.direction_buttons.remove(DirectionButtons::LEFT),
            Button::Up => self.direction_buttons.remove(DirectionButtons::UP),
            Button::Right => self.direction_buttons.remove(DirectionButtons::RIGHT),
            Button::Down => self.direction_buttons.remove(DirectionButtons::DOWN),
            Button::A => self.action_buttons.remove(ActionButtons::A),
            Button::B => self.action_buttons.remove(ActionButtons::B),
            Button::Select => self.action_buttons.remove(ActionButtons::SELECT),
            Button::Start => self.action_buttons.remove(ActionButtons::START),
        }
    }

    pub const fn read(&self) -> u8 {
        let action_buttons_bits = if self.actions_enabled {
            self.action_buttons.bits() | (1 << 5)
        } else {
            0
        };

        let direction_buttons_bits = if self.directions_enabled {
            self.direction_buttons.bits() | (1 << 4)
        } else {
            0
        };

        // pressed on low
        !(action_buttons_bits | direction_buttons_bits)
    }

    pub fn write(&mut self, val: u8) {
        self.actions_enabled = ((val >> 5) & 1) == 0;
        self.directions_enabled = ((val >> 4) & 1) == 0;
    }
}
