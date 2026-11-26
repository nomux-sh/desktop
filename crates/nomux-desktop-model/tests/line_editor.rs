//! The line of text of the rename dialog.

use nomux_desktop_model::LineEditor;

#[test]
fn typing_inserts_at_the_cursor_and_backspace_removes_whole_characters() {
    let mut line = LineEditor::new("dév");
    assert_eq!(line.split(), ("dév", ""));
    line.left();
    line.left();
    line.insert("x");
    assert_eq!(line.text(), "dxév");
    assert_eq!(line.split(), ("dx", "év"));
    line.backspace();
    line.backspace();
    assert_eq!(line.text(), "év");
    // At the start there is nothing before the cursor.
    line.backspace();
    assert_eq!(line.text(), "év");
    line.delete();
    assert_eq!(line.text(), "v");
}

#[test]
fn the_cursor_stays_inside_the_line() {
    let mut line = LineEditor::new("ab");
    line.right();
    assert_eq!(line.split(), ("ab", ""));
    line.home();
    line.left();
    assert_eq!(line.split(), ("", "ab"));
    line.end();
    assert_eq!(line.split(), ("ab", ""));
    line.delete();
    assert_eq!(line.text(), "ab");
}

#[test]
fn a_control_character_never_enters_the_line() {
    let mut line = LineEditor::default();
    line.insert("a\nb\t\u{7f}c");
    assert_eq!(line.text(), "abc");
}
