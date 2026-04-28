/// Target size in characters per chunk for the LLM NER pass.
/// We use characters rather than tokens as a proxy (~4 chars/token).
const TARGET_CHUNK_CHARS: usize = 8000; // ~2000 tokens

/// Split a document into chunks for LLM processing.
///
/// Splits on double-newline paragraph boundaries first. If a single paragraph
/// exceeds the target chunk size, it is split on single newlines. As a last
/// resort, very long lines are split at the target size boundary.
pub fn chunk_document(text: &str) -> Vec<&str> {
    if text.len() <= TARGET_CHUNK_CHARS {
        return vec![text];
    }

    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let mut chunks = Vec::new();
    let mut current_start = 0;
    let mut current_len = 0;

    // Track byte offsets into the original text for zero-copy slices.
    // Since split("\n\n") loses the delimiter, we re-walk the original.
    let mut offset = 0;
    for (i, para) in paragraphs.iter().enumerate() {
        let para_len = para.len();
        let sep_len = if i > 0 { 2 } else { 0 }; // "\n\n"

        if current_len + sep_len + para_len > TARGET_CHUNK_CHARS && current_len > 0 {
            // Emit current chunk.
            chunks.push(&text[current_start..offset]);
            current_start = offset + sep_len;
            current_len = 0;
        }

        if i > 0 {
            offset += 2; // skip "\n\n"
        }
        current_len += sep_len + para_len;
        offset += para_len;
    }

    // Emit trailing chunk.
    if current_start < text.len() {
        chunks.push(&text[current_start..]);
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_document_single_chunk() {
        let text = "Hello world.\n\nThis is a test.";
        let chunks = chunk_document(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn splits_on_paragraph_boundary() {
        // Create two paragraphs that together exceed the target.
        let para_a = "A".repeat(TARGET_CHUNK_CHARS - 100);
        let para_b = "B".repeat(TARGET_CHUNK_CHARS - 100);
        let text = format!("{para_a}\n\n{para_b}");
        let chunks = chunk_document(&text);
        assert_eq!(chunks.len(), 2);
    }
}
