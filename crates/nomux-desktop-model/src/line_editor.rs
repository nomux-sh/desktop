//! A line of text with a cursor: the field of the rename of a session.

/// The text and the position of the cursor, in characters.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LineEditor {
    text: String,
    cursor: usize,
}

impl LineEditor {
    /// A line with `text`, the cursor at its end.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            cursor: text.chars().count(),
        }
    }

    /// The text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The text before and after the cursor.
    #[must_use]
    pub fn split(&self) -> (&str, &str) {
        self.text.split_at(self.byte_index(self.cursor))
    }

    fn byte_index(&self, characters: usize) -> usize {
        self.text
            .char_indices()
            .nth(characters)
            .map_or(self.text.len(), |(index, _)| index)
    }

    /// Inserts `text` at the cursor. A control character is dropped.
    pub fn insert(&mut self, text: &str) {
        let clean: String = text
            .chars()
            .filter(|character| !character.is_control())
            .collect();
        self.text.insert_str(self.byte_index(self.cursor), &clean);
        self.cursor += clean.chars().count();
    }

    /// Deletes the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        let start = self.byte_index(self.cursor);
        let end = self.byte_index(self.cursor + 1);
        self.text.replace_range(start..end, "");
    }

    /// Deletes the character after the cursor.
    pub fn delete(&mut self) {
        let start = self.byte_index(self.cursor);
        let end = self.byte_index(self.cursor + 1);
        self.text.replace_range(start..end, "");
    }

    /// Moves the cursor one character to the left.
    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    /// Moves the cursor one character to the right.
    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.text.chars().count());
    }

    /// Moves the cursor to the start of the line.
    pub fn home(&mut self) {
        self.cursor = 0;
    }

    /// Moves the cursor to the end of the line.
    pub fn end(&mut self) {
        self.cursor = self.text.chars().count();
    }
}
