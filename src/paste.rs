// paste.rs - pure text-chunking logic for the paste() dispatcher.
//
// Splits text into a sequence of chunks: single printable-ASCII characters
// (typed directly via the uinput daemon, layout-sensitive) and runs of
// non-ASCII characters (batched into one clipboard write + one Ctrl+V paste
// each, to minimize clipboard churn and avoid Wayland async race
// conditions). Kept as a pure function with no I/O so it's unit-testable in
// isolation.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Chunk {
    Ascii(u8),
    NonAscii(String),
}

pub fn chunk_for_typing(text: &str) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut buf = String::new();

    for ch in text.chars() {
        let code = ch as u32;
        if (32..=126).contains(&code) {
            if !buf.is_empty() {
                chunks.push(Chunk::NonAscii(std::mem::take(&mut buf)));
            }
            chunks.push(Chunk::Ascii(code as u8));
        } else {
            buf.push(ch);
        }
    }

    if !buf.is_empty() {
        chunks.push(Chunk::NonAscii(buf));
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_produces_no_chunks() {
        assert_eq!(chunk_for_typing(""), vec![]);
    }

    #[test]
    fn all_ascii_produces_one_chunk_per_char() {
        let chunks = chunk_for_typing("abc");
        assert_eq!(
            chunks,
            vec![Chunk::Ascii(b'a'), Chunk::Ascii(b'b'), Chunk::Ascii(b'c'),]
        );
    }

    #[test]
    fn all_non_ascii_produces_one_batched_chunk() {
        let chunks = chunk_for_typing("æøå");
        assert_eq!(chunks, vec![Chunk::NonAscii("æøå".to_string())]);
    }

    #[test]
    fn alternating_ascii_and_non_ascii_batches_runs() {
        let chunks = chunk_for_typing("aæøb");
        assert_eq!(
            chunks,
            vec![
                Chunk::Ascii(b'a'),
                Chunk::NonAscii("æø".to_string()),
                Chunk::Ascii(b'b'),
            ]
        );
    }

    #[test]
    fn trailing_non_ascii_run_is_flushed() {
        let chunks = chunk_for_typing("aæø");
        assert_eq!(
            chunks,
            vec![Chunk::Ascii(b'a'), Chunk::NonAscii("æø".to_string())]
        );
    }

    #[test]
    fn control_and_del_are_treated_as_non_ascii() {
        // Only 32-126 is typed directly; everything else (control chars,
        // DEL, and non-ASCII) goes through the clipboard batching path -
        // matches keymap_lookup() returning -1 for those same codes.
        let chunks = chunk_for_typing("a\u{7f}b");
        assert_eq!(
            chunks,
            vec![
                Chunk::Ascii(b'a'),
                Chunk::NonAscii("\u{7f}".to_string()),
                Chunk::Ascii(b'b'),
            ]
        );
    }
}
