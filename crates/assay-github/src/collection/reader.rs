use std::io::{self, Read};

use serde::Deserialize;

use crate::{
    collection::error::{CollectionError, CollectionErrorKind, CollectionStage},
    http::GitHubResponse,
};

const LIMIT_MARKER: &str = "github_response_limit_exceeded";

pub(crate) struct LimitedReader<R> {
    inner: R,
    remaining: usize,
}

impl<R> LimitedReader<R> {
    pub(crate) const fn new(inner: R, limit: usize) -> Self {
        Self {
            inner,
            remaining: limit,
        }
    }
}

impl<R: Read> Read for LimitedReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0_u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => Err(io::Error::other(LIMIT_MARKER)),
            };
        }
        let permitted = self.remaining.min(buffer.len());
        let read = self.inner.read(&mut buffer[..permitted])?;
        self.remaining -= read;
        Ok(read)
    }
}

pub(crate) fn parse_json_limited<T: for<'de> Deserialize<'de>>(
    response: GitHubResponse,
    limit: usize,
    stage: CollectionStage,
) -> Result<T, CollectionError> {
    if content_length_exceeds(&response, limit) {
        return Err(CollectionError::new(
            CollectionErrorKind::ResponseLimit,
            stage,
        ));
    }
    let reader = LimitedReader::new(response.into_body(), limit);
    serde_json::from_reader(reader).map_err(|error| {
        let kind = if error.to_string().contains(LIMIT_MARKER) {
            CollectionErrorKind::ResponseLimit
        } else {
            CollectionErrorKind::InvalidProviderResponse
        };
        CollectionError::new(kind, stage)
    })
}

pub(crate) fn content_length_exceeds(response: &GitHubResponse, limit: usize) -> bool {
    response
        .header("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > limit)
}

pub(crate) fn is_response_limit(error: &serde_json::Error) -> bool {
    error.to_string().contains(LIMIT_MARKER)
}

pub(crate) fn percent_encode_path_segment(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::new();
    for &byte in bytes {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            output.push(char::from(byte));
        } else {
            output.push('%');
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    output
}
