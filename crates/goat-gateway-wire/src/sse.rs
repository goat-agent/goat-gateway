use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub event: Option<String>,
    pub data: String,
}

impl Frame {
    pub fn encode(event: &str, data: &Value) -> String {
        format!("event: {event}\ndata: {data}\n\n")
    }

    pub fn json(&self) -> Option<Value> {
        serde_json::from_str(&self.data).ok()
    }
}

#[derive(Debug, Default)]
pub struct Parser {
    buffer: Vec<u8>,
}

impl Parser {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<Frame> {
        self.buffer.extend_from_slice(chunk);
        let mut frames = Vec::new();

        while let Some(end) = find_boundary(&self.buffer) {
            let raw = self.buffer.drain(..end.consumed).collect::<Vec<_>>();
            let text = String::from_utf8_lossy(&raw[..end.len]).into_owned();
            if let Some(frame) = parse(&text) {
                frames.push(frame);
            }
        }
        frames
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

struct Boundary {
    len: usize,
    consumed: usize,
}

fn find_boundary(buffer: &[u8]) -> Option<Boundary> {
    for (index, window) in buffer.windows(2).enumerate() {
        if window == b"\n\n" {
            return Some(Boundary {
                len: index,
                consumed: index + 2,
            });
        }
    }
    for (index, window) in buffer.windows(4).enumerate() {
        if window == b"\r\n\r\n" {
            return Some(Boundary {
                len: index,
                consumed: index + 4,
            });
        }
    }
    None
}

fn parse(text: &str) -> Option<Frame> {
    let mut event = None;
    let mut data = String::new();

    for line in text.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some(rest) = line.strip_prefix("event:") {
            event = Some(rest.trim().to_owned());
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }

    if event.is_none() && data.is_empty() {
        None
    } else {
        Some(Frame { event, data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_split_on_blank_lines() {
        let mut parser = Parser::default();
        let frames = parser.push(b"event: a\ndata: {\"x\":1}\n\nevent: b\ndata: {}\n\n");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].event.as_deref(), Some("a"));
        assert_eq!(frames[0].json().unwrap()["x"], 1);
        assert_eq!(frames[1].event.as_deref(), Some("b"));
    }

    #[test]
    fn a_frame_split_across_chunks_is_reassembled() {
        let mut parser = Parser::default();
        assert!(parser.push(b"event: a\nda").is_empty());
        assert!(parser.push(b"ta: {\"x\":").is_empty());
        let frames = parser.push(b"1}\n\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].json().unwrap()["x"], 1);
    }

    #[test]
    fn multi_line_data_is_joined_with_newlines() {
        let mut parser = Parser::default();
        let frames = parser.push(b"event: a\ndata: one\ndata: two\n\n");
        assert_eq!(frames[0].data, "one\ntwo");
    }

    #[test]
    fn carriage_returns_are_tolerated() {
        let mut parser = Parser::default();
        let frames = parser.push(b"event: a\r\ndata: {}\r\n\r\n");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].event.as_deref(), Some("a"));
    }
}
