//! Mapping between `roxy_keyboard_input::KeyCode` and Linux `KEY_*` evdev codes.
//!
//! The mapping is a large flat `match` over the 104-key `KeyCode` enum.  All codes use the
//! constants from `roxy_evdev_types::codes` so that the symbolic name is visible next to the
//! numeric value.

use roxy_keyboard_input::KeyCode;

use roxy_evdev_types::{
    KEY_0, KEY_1, KEY_2, KEY_3, KEY_4, KEY_5, KEY_6, KEY_7, KEY_8, KEY_9, KEY_A, KEY_APOSTROPHE,
    KEY_B, KEY_BACKSLASH, KEY_BACKSPACE, KEY_C, KEY_CAPSLOCK, KEY_COMMA, KEY_D, KEY_DELETE,
    KEY_DOT, KEY_DOWN, KEY_E, KEY_END, KEY_ENTER, KEY_EQUAL, KEY_ESC, KEY_F, KEY_F1, KEY_F2,
    KEY_F3, KEY_F4, KEY_F5, KEY_F6, KEY_F7, KEY_F8, KEY_F9, KEY_F10, KEY_F11, KEY_F12, KEY_G,
    KEY_GRAVE, KEY_H, KEY_HOME, KEY_I, KEY_INSERT, KEY_J, KEY_K, KEY_KP0, KEY_KP1, KEY_KP2,
    KEY_KP3, KEY_KP4, KEY_KP5, KEY_KP6, KEY_KP7, KEY_KP8, KEY_KP9, KEY_KPASTERISK, KEY_KPDOT,
    KEY_KPENTER, KEY_KPMINUS, KEY_KPPLUS, KEY_KPSLASH, KEY_L, KEY_LEFT, KEY_LEFTALT, KEY_LEFTBRACE,
    KEY_LEFTCTRL, KEY_LEFTMETA, KEY_LEFTSHIFT, KEY_M, KEY_MENU, KEY_MINUS, KEY_N, KEY_NUMLOCK,
    KEY_O, KEY_P, KEY_PAGEDOWN, KEY_PAGEUP, KEY_PAUSE, KEY_Q, KEY_R, KEY_RIGHT, KEY_RIGHTALT,
    KEY_RIGHTBRACE, KEY_RIGHTCTRL, KEY_RIGHTMETA, KEY_RIGHTSHIFT, KEY_S, KEY_SCROLLLOCK,
    KEY_SEMICOLON, KEY_SLASH, KEY_SPACE, KEY_SYSRQ, KEY_T, KEY_TAB, KEY_U, KEY_UP, KEY_V, KEY_W,
    KEY_X, KEY_Y, KEY_Z,
};

/// Converts a Roxy `KeyCode` to the Linux `KEY_*` evdev code.
///
/// # Panics
///
/// Panics when the `KeyCode` variant has no mapping (should never happen for the current
/// 104-key set).
#[must_use]
pub fn keycode_to_evdev(code: KeyCode) -> u16 {
    function_row(code)
        .or_else(|| number_row(code))
        .or_else(|| letter_rows(code))
        .or_else(|| modifier_row(code))
        .or_else(|| editing_and_numpad(code))
        .expect("every KeyCode variant maps to a Linux KEY_* code")
}

fn function_row(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::Escape => KEY_ESC,
        KeyCode::F1 => KEY_F1,
        KeyCode::F2 => KEY_F2,
        KeyCode::F3 => KEY_F3,
        KeyCode::F4 => KEY_F4,
        KeyCode::F5 => KEY_F5,
        KeyCode::F6 => KEY_F6,
        KeyCode::F7 => KEY_F7,
        KeyCode::F8 => KEY_F8,
        KeyCode::F9 => KEY_F9,
        KeyCode::F10 => KEY_F10,
        KeyCode::F11 => KEY_F11,
        KeyCode::F12 => KEY_F12,
        KeyCode::PrintScreen => KEY_SYSRQ,
        KeyCode::ScrollLock => KEY_SCROLLLOCK,
        KeyCode::PauseBreak => KEY_PAUSE,
        _ => return None,
    })
}

fn number_row(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::Backquote => KEY_GRAVE,
        KeyCode::Digit1 => KEY_1,
        KeyCode::Digit2 => KEY_2,
        KeyCode::Digit3 => KEY_3,
        KeyCode::Digit4 => KEY_4,
        KeyCode::Digit5 => KEY_5,
        KeyCode::Digit6 => KEY_6,
        KeyCode::Digit7 => KEY_7,
        KeyCode::Digit8 => KEY_8,
        KeyCode::Digit9 => KEY_9,
        KeyCode::Digit0 => KEY_0,
        KeyCode::Minus => KEY_MINUS,
        KeyCode::Equals => KEY_EQUAL,
        KeyCode::Backspace => KEY_BACKSPACE,
        _ => return None,
    })
}

fn letter_rows(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::Tab => KEY_TAB,
        KeyCode::Q => KEY_Q,
        KeyCode::W => KEY_W,
        KeyCode::E => KEY_E,
        KeyCode::R => KEY_R,
        KeyCode::T => KEY_T,
        KeyCode::Y => KEY_Y,
        KeyCode::U => KEY_U,
        KeyCode::I => KEY_I,
        KeyCode::O => KEY_O,
        KeyCode::P => KEY_P,
        KeyCode::BracketLeft => KEY_LEFTBRACE,
        KeyCode::BracketRight => KEY_RIGHTBRACE,
        KeyCode::Backslash => KEY_BACKSLASH,
        KeyCode::CapsLock => KEY_CAPSLOCK,
        KeyCode::A => KEY_A,
        KeyCode::S => KEY_S,
        KeyCode::D => KEY_D,
        KeyCode::F => KEY_F,
        KeyCode::G => KEY_G,
        KeyCode::H => KEY_H,
        KeyCode::J => KEY_J,
        KeyCode::K => KEY_K,
        KeyCode::L => KEY_L,
        KeyCode::Semicolon => KEY_SEMICOLON,
        KeyCode::Apostrophe => KEY_APOSTROPHE,
        KeyCode::LeftShift => KEY_LEFTSHIFT,
        KeyCode::Z => KEY_Z,
        KeyCode::X => KEY_X,
        KeyCode::C => KEY_C,
        KeyCode::V => KEY_V,
        KeyCode::B => KEY_B,
        KeyCode::N => KEY_N,
        KeyCode::M => KEY_M,
        KeyCode::Comma => KEY_COMMA,
        KeyCode::Period => KEY_DOT,
        KeyCode::Slash => KEY_SLASH,
        KeyCode::RightShift => KEY_RIGHTSHIFT,
        _ => return None,
    })
}

fn modifier_row(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::LeftCtrl => KEY_LEFTCTRL,
        KeyCode::LeftSuper => KEY_LEFTMETA,
        KeyCode::LeftAlt => KEY_LEFTALT,
        KeyCode::Space => KEY_SPACE,
        KeyCode::RightAlt => KEY_RIGHTALT,
        KeyCode::RightSuper => KEY_RIGHTMETA,
        KeyCode::Menu => KEY_MENU,
        KeyCode::RightCtrl => KEY_RIGHTCTRL,
        _ => return None,
    })
}

fn editing_and_numpad(code: KeyCode) -> Option<u16> {
    Some(match code {
        KeyCode::Insert => KEY_INSERT,
        KeyCode::Home => KEY_HOME,
        KeyCode::PageUp => KEY_PAGEUP,
        KeyCode::Delete => KEY_DELETE,
        KeyCode::End => KEY_END,
        KeyCode::PageDown => KEY_PAGEDOWN,
        KeyCode::Return => KEY_ENTER,
        KeyCode::ArrowUp => KEY_UP,
        KeyCode::ArrowDown => KEY_DOWN,
        KeyCode::ArrowLeft => KEY_LEFT,
        KeyCode::ArrowRight => KEY_RIGHT,
        KeyCode::NumpadLock => KEY_NUMLOCK,
        KeyCode::NumpadDivide => KEY_KPSLASH,
        KeyCode::NumpadMultiply => KEY_KPASTERISK,
        KeyCode::NumpadSubtract => KEY_KPMINUS,
        KeyCode::NumpadAdd => KEY_KPPLUS,
        KeyCode::NumpadEnter => KEY_KPENTER,
        KeyCode::NumpadDecimal => KEY_KPDOT,
        KeyCode::Numpad0 => KEY_KP0,
        KeyCode::Numpad1 => KEY_KP1,
        KeyCode::Numpad2 => KEY_KP2,
        KeyCode::Numpad3 => KEY_KP3,
        KeyCode::Numpad4 => KEY_KP4,
        KeyCode::Numpad5 => KEY_KP5,
        KeyCode::Numpad6 => KEY_KP6,
        KeyCode::Numpad7 => KEY_KP7,
        KeyCode::Numpad8 => KEY_KP8,
        KeyCode::Numpad9 => KEY_KP9,
        _ => return None,
    })
}

/// Returns a static slice of every `KEY_*` code that the keyboard device supports.
#[must_use]
pub fn supported_key_codes() -> &'static [u16] {
    SUPPORTED_KEY_CODES
}

const SUPPORTED_KEY_CODES: &[u16] = &[
    KEY_ESC,
    KEY_F1,
    KEY_F2,
    KEY_F3,
    KEY_F4,
    KEY_F5,
    KEY_F6,
    KEY_F7,
    KEY_F8,
    KEY_F9,
    KEY_F10,
    KEY_F11,
    KEY_F12,
    KEY_SYSRQ,
    KEY_SCROLLLOCK,
    KEY_PAUSE,
    KEY_GRAVE,
    KEY_1,
    KEY_2,
    KEY_3,
    KEY_4,
    KEY_5,
    KEY_6,
    KEY_7,
    KEY_8,
    KEY_9,
    KEY_0,
    KEY_MINUS,
    KEY_EQUAL,
    KEY_BACKSPACE,
    KEY_TAB,
    KEY_Q,
    KEY_W,
    KEY_E,
    KEY_R,
    KEY_T,
    KEY_Y,
    KEY_U,
    KEY_I,
    KEY_O,
    KEY_P,
    KEY_LEFTBRACE,
    KEY_RIGHTBRACE,
    KEY_BACKSLASH,
    KEY_CAPSLOCK,
    KEY_A,
    KEY_S,
    KEY_D,
    KEY_F,
    KEY_G,
    KEY_H,
    KEY_J,
    KEY_K,
    KEY_L,
    KEY_SEMICOLON,
    KEY_APOSTROPHE,
    KEY_LEFTSHIFT,
    KEY_Z,
    KEY_X,
    KEY_C,
    KEY_V,
    KEY_B,
    KEY_N,
    KEY_M,
    KEY_COMMA,
    KEY_DOT,
    KEY_SLASH,
    KEY_RIGHTSHIFT,
    KEY_LEFTCTRL,
    KEY_LEFTMETA,
    KEY_LEFTALT,
    KEY_SPACE,
    KEY_RIGHTALT,
    KEY_RIGHTMETA,
    KEY_MENU,
    KEY_RIGHTCTRL,
    KEY_INSERT,
    KEY_HOME,
    KEY_PAGEUP,
    KEY_DELETE,
    KEY_END,
    KEY_PAGEDOWN,
    KEY_ENTER,
    KEY_UP,
    KEY_DOWN,
    KEY_LEFT,
    KEY_RIGHT,
    KEY_NUMLOCK,
    KEY_KPSLASH,
    KEY_KPASTERISK,
    KEY_KPMINUS,
    KEY_KPPLUS,
    KEY_KPENTER,
    KEY_KPDOT,
    KEY_KP0,
    KEY_KP1,
    KEY_KP2,
    KEY_KP3,
    KEY_KP4,
    KEY_KP5,
    KEY_KP6,
    KEY_KP7,
    KEY_KP8,
    KEY_KP9,
];
