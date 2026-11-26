//! The bytes of keys, paste and mouse for the modes of the program.

use gpui::{Keystroke, Modifiers};
use nomux_term::Modes;
use nomux_term_view::GridPoint;
use nomux_term_view::input::{
    MouseAction, MouseReport, alternate_scroll_bytes, key_bytes, mouse_bytes, paste_bytes,
};

fn key(text: &'static str, modes: Modes) -> Option<String> {
    let keystroke = Keystroke::parse(text).expect("a valid keystroke");
    key_bytes(&keystroke, modes).map(|bytes| String::from_utf8(bytes).expect("UTF-8"))
}

#[test]
fn cursor_keys_follow_the_application_cursor_mode_and_the_modifiers() {
    assert_eq!(key("up", Modes::empty()).as_deref(), Some("\x1b[A"));
    assert_eq!(
        key("up", Modes::APPLICATION_CURSOR).as_deref(),
        Some("\x1bOA")
    );
    assert_eq!(
        key("ctrl-left", Modes::APPLICATION_CURSOR).as_deref(),
        Some("\x1b[1;5D")
    );
    assert_eq!(
        key("shift-home", Modes::empty()).as_deref(),
        Some("\x1b[1;2H")
    );
}

#[test]
fn function_and_editing_keys_use_the_xterm_codes() {
    assert_eq!(key("f1", Modes::empty()).as_deref(), Some("\x1bOP"));
    assert_eq!(key("f10", Modes::empty()).as_deref(), Some("\x1b[21~"));
    assert_eq!(
        key("ctrl-f5", Modes::empty()).as_deref(),
        Some("\x1b[15;5~")
    );
    assert_eq!(key("delete", Modes::empty()).as_deref(), Some("\x1b[3~"));
    assert_eq!(key("shift-tab", Modes::empty()).as_deref(), Some("\x1b[Z"));
}

#[test]
fn control_and_alt_make_bytes_and_plain_text_goes_to_the_input_handler() {
    assert_eq!(key("ctrl-c", Modes::empty()).as_deref(), Some("\x03"));
    assert_eq!(key("ctrl-[", Modes::empty()).as_deref(), Some("\x1b"));
    assert_eq!(key("alt-b", Modes::empty()).as_deref(), Some("\x1bb"));
    assert_eq!(
        key("ctrl-alt-d", Modes::empty()).as_deref(),
        Some("\x1b\x04")
    );
    assert_eq!(key("a", Modes::empty()), None);
    // The platform modifier belongs to the application (copy, paste).
    assert_eq!(key("cmd-c", Modes::empty()), None);
    assert_eq!(key("cmd-up", Modes::empty()), None);
}

#[test]
fn a_bracketed_paste_cannot_end_early() {
    let text = "echo one\nrm -rf x\x1b[201~\n";
    assert_eq!(
        paste_bytes(text, Modes::empty()),
        b"echo one\rrm -rf x\x1b[201~\r"
    );
    assert_eq!(
        paste_bytes(text, Modes::BRACKETED_PASTE),
        b"\x1b[200~echo one\rrm -rf x\r\x1b[201~"
    );
}

#[test]
fn mouse_reports_follow_the_encoding_and_the_tracking_mode() {
    let at = |action| MouseReport {
        action,
        cell: GridPoint { row: 4, column: 9 },
        modifiers: Modifiers::default(),
    };
    assert_eq!(mouse_bytes(at(MouseAction::Press(0)), Modes::empty()), None);
    let sgr = Modes::MOUSE_CLICK | Modes::MOUSE_SGR;
    assert_eq!(
        mouse_bytes(at(MouseAction::Press(0)), sgr).as_deref(),
        Some(&b"\x1b[<0;10;5M"[..])
    );
    assert_eq!(
        mouse_bytes(at(MouseAction::Release(0)), sgr).as_deref(),
        Some(&b"\x1b[<0;10;5m"[..])
    );
    assert_eq!(
        mouse_bytes(at(MouseAction::WheelUp), sgr).as_deref(),
        Some(&b"\x1b[<64;10;5M"[..])
    );
    // Motion only with a button and the drag mode, or with the motion mode.
    assert_eq!(mouse_bytes(at(MouseAction::Motion(None)), sgr), None);
    let drag = Modes::MOUSE_DRAG | Modes::MOUSE_SGR;
    assert_eq!(
        mouse_bytes(at(MouseAction::Motion(Some(0))), drag).as_deref(),
        Some(&b"\x1b[<32;10;5M"[..])
    );
    // The X10 encoding: 32 + value, and the release has the code 3.
    assert_eq!(
        mouse_bytes(at(MouseAction::Release(2)), Modes::MOUSE_CLICK).as_deref(),
        Some(&[0x1b, b'[', b'M', 32 + 3, 32 + 10, 32 + 5][..])
    );
}

#[test]
fn the_wheel_on_the_alternate_screen_sends_cursor_keys_only_when_asked() {
    let alternate = Modes::ALTERNATE_SCREEN | Modes::ALTERNATE_SCROLL;
    assert_eq!(
        alternate_scroll_bytes(true, alternate).as_deref(),
        Some(&b"\x1b[A"[..])
    );
    assert_eq!(
        alternate_scroll_bytes(false, alternate | Modes::APPLICATION_CURSOR).as_deref(),
        Some(&b"\x1bOB"[..])
    );
    assert_eq!(alternate_scroll_bytes(true, Modes::ALTERNATE_SCREEN), None);
    assert_eq!(
        alternate_scroll_bytes(true, alternate | Modes::MOUSE_CLICK),
        None
    );
}
