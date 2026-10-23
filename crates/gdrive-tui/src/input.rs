use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputBuffer {
    chars: Vec<char>,
    cursor: usize,
}

impl InputBuffer {
    pub fn new(initial: impl AsRef<str>) -> Self {
        let chars: Vec<char> = initial.as_ref().chars().collect();
        let cursor = chars.len();
        Self { chars, cursor }
    }

    pub fn as_str(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn len(&self) -> usize {
        self.chars.len()
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn clear(&mut self) {
        self.chars.clear();
        self.cursor = 0;
    }

    pub fn set(&mut self, text: impl AsRef<str>) {
        self.chars = text.as_ref().chars().collect();
        self.cursor = self.chars.len();
    }

    pub fn insert(&mut self, c: char) {
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn insert_str(&mut self, s: &str) {
        for c in s.chars() {
            self.insert(c);
        }
    }

    pub fn delete_backspace(&mut self) -> bool {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.chars.remove(self.cursor);
            true
        } else {
            false
        }
    }

    pub fn delete_char(&mut self) -> bool {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
            true
        } else {
            false
        }
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        if self.cursor < self.chars.len() {
            self.cursor += 1;
        }
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.chars.len();
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('a') => {
                    self.move_home();
                    return true;
                }
                KeyCode::Char('e') => {
                    self.move_end();
                    return true;
                }
                KeyCode::Char('k') => {
                    self.chars.truncate(self.cursor);
                    return true;
                }
                KeyCode::Char('u') => {
                    self.chars.drain(0..self.cursor);
                    self.cursor = 0;
                    return true;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Left => {
                self.move_left();
                true
            }
            KeyCode::Right => {
                self.move_right();
                true
            }
            KeyCode::Home => {
                self.move_home();
                true
            }
            KeyCode::End => {
                self.move_end();
                true
            }
            KeyCode::Backspace => self.delete_backspace(),
            KeyCode::Delete => self.delete_char(),
            KeyCode::Char(c) => {
                self.insert(c);
                true
            }
            _ => false,
        }
    }

    /// Returns (before_cursor, cursor_char, after_cursor)
    pub fn cursor_parts(&self) -> (String, Option<char>, String) {
        let before: String = self.chars[..self.cursor].iter().collect();
        let current = self.chars.get(self.cursor).copied();
        let after: String = if self.cursor < self.chars.len() {
            self.chars[self.cursor + 1..].iter().collect()
        } else {
            String::new()
        };
        (before, current, after)
    }

    /// Returns rendered representation with visual cursor
    pub fn display_string(&self) -> String {
        let (before, current, after) = self.cursor_parts();
        if let Some(c) = current {
            format!("{}[{}]{}", before, c, after)
        } else {
            format!("{}_", before)
        }
    }
    /// Autocomplete local directory or file path based on current buffer string
    pub fn autocomplete_local_path(&mut self) -> bool {
        let current = self.as_str();
        let path = std::path::Path::new(&current);
        let (dir, prefix) = if current.ends_with('/') {
            (path, "")
        } else {
            let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let file_prefix = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            (parent, file_prefix)
        };

        if let Ok(entries) = std::fs::read_dir(dir) {
            let matches: Vec<String> = entries
                .filter_map(|e| e.ok())
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with(prefix) && !name.starts_with('.') {
                        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        let p = if dir == std::path::Path::new(".") || dir.as_os_str().is_empty() {
                            if is_dir {
                                format!("{}/", name)
                            } else {
                                name
                            }
                        } else {
                            let joined = dir.join(&name);
                            if is_dir {
                                format!("{}/", joined.display())
                            } else {
                                joined.display().to_string()
                            }
                        };
                        Some(p)
                    } else {
                        None
                    }
                })
                .collect();

            if matches.len() == 1 {
                self.set(&matches[0]);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_buffer_typing_and_cursor() {
        let mut buf = InputBuffer::new("hello");
        assert_eq!(buf.as_str(), "hello");
        assert_eq!(buf.cursor(), 5);

        // Move left twice
        buf.move_left();
        buf.move_left();
        assert_eq!(buf.cursor(), 3);

        // Insert char in middle
        buf.insert('-');
        assert_eq!(buf.as_str(), "hel-lo");
        assert_eq!(buf.cursor(), 4);

        // Backspace
        buf.delete_backspace();
        assert_eq!(buf.as_str(), "hello");
        assert_eq!(buf.cursor(), 3);

        // Delete forward
        buf.delete_char();
        assert_eq!(buf.as_str(), "helo");
        assert_eq!(buf.cursor(), 3);

        // Home and End
        buf.move_home();
        assert_eq!(buf.cursor(), 0);
        buf.move_end();
        assert_eq!(buf.cursor(), 4);
    }

    #[test]
    fn test_input_buffer_unicode() {
        let mut buf = InputBuffer::new("Dokumen 📁.md");
        assert_eq!(buf.len(), 12);
        buf.move_left();
        buf.move_left();
        buf.move_left();
        assert_eq!(buf.cursor(), 9);
        buf.insert('⭐');
        assert_eq!(buf.as_str(), "Dokumen 📁⭐.md");
    }
}
