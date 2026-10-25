const MAX_CHARS: usize = 120;
const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Turns a media title into a safe Windows file name stem (no extension).
pub fn sanitize_filename(title: &str) -> String {
    let replaced: String = title
        .chars()
        .map(|c| if c.is_control() || "<>:\"/\\|?*".contains(c) { ' ' } else { c })
        .collect();
    let collapsed = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated: String = collapsed.chars().take(MAX_CHARS).collect();
    let trimmed = truncated.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        return "lumi".into();
    }
    let (stem, rest) = match trimmed.find('.') {
        Some(i) => trimmed.split_at(i),
        None => (trimmed, ""),
    };
    let stem_trim = stem.trim_end();
    if RESERVED.contains(&stem_trim.to_uppercase().as_str()) {
        format!("{stem}_{rest}")
    } else {
        trimmed.to_string()
    }
}

#[tauri::command]
pub fn default_filename(title: String, ext: String) -> String {
    let ext: String = ext
        .chars()
        .filter(|c| !c.is_control() && !c.is_whitespace() && !"<>:\"/\\|?*".contains(*c))
        .collect();
    let ext = ext.trim_matches('.');
    let stem = sanitize_filename(&title);
    if ext.is_empty() { stem } else { format!("{stem}.{ext}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_keeps_unicode_drops_forbidden() {
        let s = sanitize_filename("Nhạc chill 🎧 / tối?");
        assert!(!s.contains('/') && !s.contains('?'));
        assert!(s.contains("Nhạc") && s.contains("tối") && s.contains('🎧'));
    }

    #[test]
    fn sanitize_empty_and_reserved() {
        assert_eq!(sanitize_filename(""), "lumi");
        assert_eq!(sanitize_filename(" . "), "lumi");
        assert_eq!(sanitize_filename("CON"), "CON_");
        assert_eq!(sanitize_filename("con"), "con_");
        assert_eq!(sanitize_filename("con.txt"), "con_.txt");
        assert_eq!(sanitize_filename("COM1"), "COM1_");
        assert_eq!(sanitize_filename("CONSOLE"), "CONSOLE");
    }

    #[test]
    fn sanitize_truncates_by_chars() {
        let s = sanitize_filename(&"🎧".repeat(500));
        assert_eq!(s.chars().count(), 120);
        let s = sanitize_filename(&"ạ".repeat(500));
        assert!(s.chars().count() <= 120);
    }

    #[test]
    fn sanitize_trailing_dots_and_control() {
        let s = sanitize_filename("a. . ");
        assert!(!s.ends_with('.') && !s.ends_with(' '));
        assert!(!sanitize_filename("a\nb\t\u{7}c").chars().any(|c| c.is_control()));
        let s = sanitize_filename(&format!("{}. ", "a".repeat(119)));
        assert!(!s.ends_with('.') && !s.ends_with(' '));
    }

    #[test]
    fn default_filename_joins_ext() {
        assert_eq!(default_filename("a/b".into(), "mp3".into()), "a b.mp3");
        assert_eq!(default_filename("t".into(), ".mp3".into()), "t.mp3");
        assert_eq!(default_filename("t".into(), "..\\m/p3".into()), "t.mp3");
        assert_eq!(default_filename("t".into(), "".into()), "t");
        assert_eq!(default_filename("t".into(), ".".into()), "t");
    }
}
