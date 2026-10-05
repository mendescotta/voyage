use std::io::{self, BufRead};
use std::path::Path;

/// Reads a text file, replacing bytes that are not valid UTF-8 instead of failing.
pub fn read_text_lossy(path: &Path) -> io::Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(path)?).into_owned())
}

/// Like `BufRead::lines`, but invalid UTF-8 is replaced rather than ending the stream.
pub fn lossy_lines<R: BufRead>(reader: R) -> impl Iterator<Item = io::Result<String>> {
    reader.split(b'\n').map(|chunk| {
        chunk.map(|mut bytes| {
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            String::from_utf8_lossy(&bytes).into_owned()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn read_text_lossy_keeps_a_log_with_invalid_utf8() {
        let path = std::env::temp_dir().join(format!("voyage-text-{}.log", std::process::id()));
        std::fs::write(&path, b"line one\nbad \xff\xfe bytes\nline three\n").unwrap();
        let text = read_text_lossy(&path).unwrap();
        std::fs::remove_file(&path).ok();
        assert!(text.contains("line one"));
        assert!(text.contains("line three"));
        assert!(text.contains('\u{FFFD}'));
    }

    #[test]
    fn read_text_lossy_reports_a_missing_file() {
        assert!(read_text_lossy(std::path::Path::new("/nonexistent/voyage.log")).is_err());
    }

    #[test]
    fn lossy_lines_does_not_stop_at_invalid_utf8() {
        let data = b"first\nsecond \xc3\x28 broken\n>>> FINISH\n".to_vec();
        let lines: Vec<String> = lossy_lines(Cursor::new(data)).map(|l| l.unwrap()).collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "first");
        assert!(lines[1].starts_with("second ") && lines[1].contains('\u{FFFD}'));
        assert_eq!(lines[2], ">>> FINISH");
    }

    #[test]
    fn lossy_lines_strips_carriage_returns_and_keeps_a_last_line_without_newline() {
        let lines: Vec<String> = lossy_lines(Cursor::new(b"a\r\nb".to_vec())).map(|l| l.unwrap()).collect();
        assert_eq!(lines, vec!["a".to_string(), "b".to_string()]);
    }
}
