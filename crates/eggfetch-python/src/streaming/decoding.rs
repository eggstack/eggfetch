//! Private incremental response decoding and line splitting.

use bytes::BytesMut;

pub(super) fn decode_bytes(encoding_name: Option<&str>, bytes: &[u8]) -> String {
    if let Some(name) = encoding_name {
        if let Some(enc) = encoding_rs::Encoding::for_label(name.as_bytes()) {
            let (decoded, _, _) = enc.decode(bytes);
            return decoded.into_owned();
        }
    }
    String::from_utf8_lossy(bytes).to_string()
}

/// Incrementally decodes chunks so multibyte characters split across network
/// boundaries are not replaced or lost.
pub(super) struct IncrementalDecoder {
    decoder: Option<encoding_rs::Decoder>,
    utf8_pending: BytesMut,
}

impl IncrementalDecoder {
    pub(super) fn new(encoding_name: Option<&str>) -> Self {
        Self {
            decoder: encoding_name
                .and_then(|name| encoding_rs::Encoding::for_label(name.as_bytes()))
                .map(encoding_rs::Encoding::new_decoder),
            utf8_pending: BytesMut::new(),
        }
    }

    pub(super) fn decode(&mut self, bytes: &[u8], last: bool) -> String {
        if let Some(decoder) = &mut self.decoder {
            // encoding_rs writes into the String's existing capacity and
            // intentionally does not reallocate for the caller.
            const MAX_INITIAL_DECODE_CAPACITY: usize = 64 * 1024;
            let capacity = bytes
                .len()
                .saturating_mul(3)
                .clamp(4, MAX_INITIAL_DECODE_CAPACITY);
            let mut output = String::with_capacity(capacity);
            let _ = decoder.decode_to_string(bytes, &mut output, last);
            return output;
        }

        self.utf8_pending.extend_from_slice(bytes);
        match std::str::from_utf8(&self.utf8_pending) {
            Ok(text) => {
                let output = text.to_owned();
                self.utf8_pending.clear();
                output
            }
            Err(error) if error.error_len().is_none() && !last => {
                let valid_up_to = error.valid_up_to();
                let output =
                    String::from_utf8_lossy(&self.utf8_pending[..valid_up_to]).into_owned();
                let _ = self.utf8_pending.split_to(valid_up_to);
                output
            }
            Err(_) => {
                let output = String::from_utf8_lossy(&self.utf8_pending).into_owned();
                self.utf8_pending.clear();
                output
            }
        }
    }

    pub(super) fn finish(&mut self) -> String {
        self.decode(&[], true)
    }
}

pub(super) fn complete_lines(buffer: &mut String) -> Vec<String> {
    let Some(last_pos) = buffer.rfind('\n') else {
        return Vec::new();
    };
    let tail = buffer.split_off(last_pos + 1);
    let complete = std::mem::replace(buffer, tail);
    complete
        .split_inclusive('\n')
        .map(|line| {
            let line = line.strip_suffix('\n').unwrap_or(line);
            line.strip_suffix('\r').unwrap_or(line).to_owned()
        })
        .collect()
}

pub(super) fn final_line(buffer: &mut String) -> Option<String> {
    if buffer.is_empty() {
        return None;
    }
    let mut line = std::mem::take(buffer);
    if line.ends_with('\r') {
        line.pop();
    }
    Some(line)
}
