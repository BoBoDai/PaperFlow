//! SSE streaming helpers.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct StreamChunk {
    #[serde(default)]
    pub(crate) choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct StreamChoice {
    #[serde(default)]
    pub(crate) delta: StreamDelta,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct StreamDelta {
    #[serde(default)]
    pub(crate) content: Option<String>,
}

pub(crate) fn next_sse_event(buffer: &str) -> Option<(usize, usize)> {
    let lf = buffer.find("\n\n").map(|index| (index, 2));
    let crlf = buffer.find("\r\n\r\n").map(|index| (index, 4));

    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

/// Holds back MiniMax-style `<think>...</think>` reasoning while streaming.
#[derive(Default)]
pub(crate) struct AnswerStream {
    raw: String,
    emitted: usize,
    answer: String,
    saw_think: bool,
    thinking_done: bool,
}

impl AnswerStream {
    pub(crate) fn push(&mut self, delta: &str) -> String {
        self.raw.push_str(delta);
        let mut out = String::new();

        if self.saw_think {
            if self.thinking_done {
                out.push_str(&self.raw[self.emitted..]);
                self.emitted = self.raw.len();
            } else if let Some(position) = self.raw[self.emitted..].find("</think") {
                let position = self.emitted + position;
                if let Some(close) = self.raw[position..].find('>') {
                    self.thinking_done = true;
                    self.emitted = position + close + 1;
                    out.push_str(&self.raw[self.emitted..]);
                    self.emitted = self.raw.len();
                }
            }
        } else if let Some(position) = self.raw.find("<think") {
            self.saw_think = true;
            out.push_str(&self.raw[self.emitted..position]);
            self.emitted = position;
        } else {
            let safe = safe_prefix_len(&self.raw);
            if safe > self.emitted {
                out.push_str(&self.raw[self.emitted..safe]);
                self.emitted = safe;
            }
        }

        self.answer.push_str(&out);
        out
    }

    pub(crate) fn finish(&self) -> String {
        if self.saw_think && !self.thinking_done {
            return String::new();
        }

        self.answer.trim().to_string()
    }
}

/// Largest prefix length that does not end with a partial `<think` tag.
fn safe_prefix_len(raw: &str) -> usize {
    const TAG: &str = "<think";
    let length = raw.len();

    for size in (1..TAG.len()).rev() {
        if raw.ends_with(&TAG[..size]) {
            return length - size;
        }
    }

    length
}
