//! The bytes that a program in a terminal gets for keys, paste, mouse and
//! focus, from the modes that the program set.
//!
//! The encodings are those of xterm. The kitty keyboard protocol is not
//! encoded in this version: a program that asks for it gets the xterm
//! encoding.

use gpui::{Keystroke, Modifiers};
use nomux_term::Modes;

use crate::selection::GridPoint;

const ESC: u8 = 0x1b;

/// The bytes of a key that is not plain text: a named key, or a key with
/// Ctrl or Alt. `None` for text: the platform gives text through the input
/// handler, also for an IME. `None` also for a key with the platform
/// modifier (Cmd, Super): the application uses those.
#[must_use]
pub fn key_bytes(keystroke: &Keystroke, modes: Modes) -> Option<Vec<u8>> {
    let modifiers = keystroke.modifiers;
    if modifiers.platform || modifiers.function {
        return None;
    }
    if let Some(bytes) = named_key(&keystroke.key, modifiers, modes) {
        return Some(bytes);
    }
    let key = single_char(&keystroke.key)?;
    if modifiers.control {
        let control = control_byte(key)?;
        return Some(with_alt(modifiers.alt, &[control]));
    }
    if modifiers.alt {
        let text = keystroke
            .key_char
            .clone()
            .unwrap_or_else(|| key.to_string());
        return Some(with_alt(true, text.as_bytes()));
    }
    None
}

/// The bytes of pasted text: line ends become carriage returns, and the
/// text is between the paste marks if the program asked for them. The end
/// mark in the text is removed, thus the text cannot end the paste early.
#[must_use]
pub fn paste_bytes(text: &str, modes: Modes) -> Vec<u8> {
    let text = text.replace("\r\n", "\r").replace('\n', "\r");
    if modes.contains(Modes::BRACKETED_PASTE) {
        let text = text.replace("\x1b[201~", "");
        [b"\x1b[200~", text.as_bytes(), b"\x1b[201~"].concat()
    } else {
        text.into_bytes()
    }
}

/// The bytes of a change of the focus, if the program asked for them.
#[must_use]
pub fn focus_bytes(focused: bool, modes: Modes) -> Option<&'static [u8]> {
    modes
        .contains(Modes::FOCUS_REPORTING)
        .then_some(if focused { b"\x1b[I" } else { b"\x1b[O" })
}

/// A mouse event on a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseReport {
    /// What happened.
    pub action: MouseAction,
    /// The cell below the pointer.
    pub cell: GridPoint,
    /// The modifiers.
    pub modifiers: Modifiers,
}

/// What a mouse did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseAction {
    /// A button went down: 0 left, 1 middle, 2 right.
    Press(u8),
    /// A button went up.
    Release(u8),
    /// The pointer moved. `Some` while a button is down.
    Motion(Option<u8>),
    /// One step of the wheel up.
    WheelUp,
    /// One step of the wheel down.
    WheelDown,
}

/// `true` if the program gets mouse events.
#[must_use]
pub fn reports_mouse(modes: Modes) -> bool {
    modes.intersects(Modes::MOUSE_CLICK | Modes::MOUSE_DRAG | Modes::MOUSE_MOTION)
}

/// The bytes of a mouse event, `None` if the program did not ask for it.
#[must_use]
pub fn mouse_bytes(report: MouseReport, modes: Modes) -> Option<Vec<u8>> {
    if !reports_mouse(modes) {
        return None;
    }
    let mut code = match report.action {
        MouseAction::Press(button) | MouseAction::Release(button) => button,
        MouseAction::Motion(button) => {
            let wanted = modes.contains(Modes::MOUSE_MOTION)
                || (button.is_some() && modes.contains(Modes::MOUSE_DRAG));
            if !wanted {
                return None;
            }
            32 + button.unwrap_or(3)
        }
        MouseAction::WheelUp => 64,
        MouseAction::WheelDown => 65,
    };
    if report.modifiers.shift {
        code += 4;
    }
    if report.modifiers.alt {
        code += 8;
    }
    if report.modifiers.control {
        code += 16;
    }
    let column = u32::from(report.cell.column) + 1;
    let row = u32::from(report.cell.row) + 1;
    let release = matches!(report.action, MouseAction::Release(_));
    if modes.contains(Modes::MOUSE_SGR) {
        let end = if release { 'm' } else { 'M' };
        return Some(format!("\x1b[<{code};{column};{row}{end}").into_bytes());
    }
    if release {
        code = 3 | (code & !3);
    }
    let mut bytes = b"\x1b[M".to_vec();
    for value in [u32::from(code), column, row] {
        let value = value + 32;
        if modes.contains(Modes::MOUSE_UTF8) {
            let character = char::from_u32(value.min(2047))?;
            let mut buffer = [0; 4];
            bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
        } else {
            bytes.push(u8::try_from(value).ok()?);
        }
    }
    Some(bytes)
}

/// The bytes of one step of the wheel on the alternate screen, when the
/// program does not get mouse events but asked for cursor keys (DEC mode
/// 1007).
#[must_use]
pub fn alternate_scroll_bytes(up: bool, modes: Modes) -> Option<Vec<u8>> {
    if reports_mouse(modes)
        || !modes.contains(Modes::ALTERNATE_SCREEN)
        || !modes.contains(Modes::ALTERNATE_SCROLL)
    {
        return None;
    }
    named_key(if up { "up" } else { "down" }, Modifiers::default(), modes)
}

fn named_key(key: &str, modifiers: Modifiers, modes: Modes) -> Option<Vec<u8>> {
    let modifier_code = 1
        + u8::from(modifiers.shift)
        + 2 * u8::from(modifiers.alt)
        + 4 * u8::from(modifiers.control);
    let application_cursor = modes.contains(Modes::APPLICATION_CURSOR);
    let bytes = match key {
        "enter" => with_alt(modifiers.alt, b"\r"),
        "tab" if modifiers.shift => b"\x1b[Z".to_vec(),
        "tab" => with_alt(modifiers.alt, b"\t"),
        "escape" => with_alt(modifiers.alt, &[ESC]),
        "backspace" if modifiers.control => with_alt(modifiers.alt, b"\x08"),
        "backspace" => with_alt(modifiers.alt, b"\x7f"),
        "space" if modifiers.control => with_alt(modifiers.alt, &[0]),
        "up" | "down" | "right" | "left" | "home" | "end" => {
            let letter = match key {
                "up" => 'A',
                "down" => 'B',
                "right" => 'C',
                "left" => 'D',
                "home" => 'H',
                _ => 'F',
            };
            cursor_key(letter, modifier_code, application_cursor)
        }
        "insert" => tilde_key(2, modifier_code),
        "delete" => tilde_key(3, modifier_code),
        "pageup" => tilde_key(5, modifier_code),
        "pagedown" => tilde_key(6, modifier_code),
        _ => function_key(key, modifier_code)?,
    };
    Some(bytes)
}

/// A cursor key: SS3 in the application cursor mode without modifiers, else
/// CSI with the modifier parameter.
fn cursor_key(letter: char, modifier_code: u8, application_cursor: bool) -> Vec<u8> {
    match (modifier_code, application_cursor) {
        (1, true) => format!("\x1bO{letter}"),
        (1, false) => format!("\x1b[{letter}"),
        (code, _) => format!("\x1b[1;{code}{letter}"),
    }
    .into_bytes()
}

fn tilde_key(number: u8, modifier_code: u8) -> Vec<u8> {
    if modifier_code == 1 {
        format!("\x1b[{number}~")
    } else {
        format!("\x1b[{number};{modifier_code}~")
    }
    .into_bytes()
}

fn function_key(key: &str, modifier_code: u8) -> Option<Vec<u8>> {
    let number: u8 = key.strip_prefix('f')?.parse().ok()?;
    let bytes = match number {
        1..=4 => {
            let letter = char::from(b'P' + number - 1);
            if modifier_code == 1 {
                format!("\x1bO{letter}").into_bytes()
            } else {
                format!("\x1b[1;{modifier_code}{letter}").into_bytes()
            }
        }
        5..=12 => {
            const CODES: [u8; 8] = [15, 17, 18, 19, 20, 21, 23, 24];
            tilde_key(CODES[usize::from(number - 5)], modifier_code)
        }
        _ => return None,
    };
    Some(bytes)
}

fn single_char(key: &str) -> Option<char> {
    let mut characters = key.chars();
    let first = characters.next()?;
    characters.next().is_none().then_some(first)
}

/// The control character of Ctrl and a key.
fn control_byte(key: char) -> Option<u8> {
    match key {
        'a'..='z' => Some(key as u8 - b'a' + 1),
        '@' | '2' => Some(0),
        '[' | '3' => Some(ESC),
        '\\' | '4' => Some(0x1c),
        ']' | '5' => Some(0x1d),
        '^' | '6' => Some(0x1e),
        '_' | '-' | '7' => Some(0x1f),
        '8' | '?' => Some(0x7f),
        _ => None,
    }
}

fn with_alt(alt: bool, bytes: &[u8]) -> Vec<u8> {
    if alt {
        [&[ESC][..], bytes].concat()
    } else {
        bytes.to_vec()
    }
}
