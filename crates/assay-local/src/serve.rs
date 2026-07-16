//! Minimal loopback HTTP dashboard for local analysis history.
//!
//! The server binds only through [`LoopbackListener`] and speaks a tiny subset
//! of HTTP/1.1 sufficient to serve the versioned local report contract. It has
//! no third-party HTTP dependency: request routing works on a `BufRead` line
//! and responses are plain bytes.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{Value, json};

use crate::history::LocalHistoryStore;
use crate::loopback::LoopbackListener;
use crate::report::LOCAL_REPORT_SCHEMA_VERSION;

/// Upper bound on the request line the server will read before rejecting it.
const MAX_REQUEST_LINE_BYTES: usize = 8 * 1024;

/// Read timeout applied to accepted connections so a stalled peer cannot pin
/// the single-threaded serial loop.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// A rendered HTTP response with a JSON body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpResponse {
    status: u16,
    reason: &'static str,
    body: Vec<u8>,
}

impl HttpResponse {
    fn json(status: u16, reason: &'static str, value: &Value) -> Self {
        let mut body = serde_json::to_vec(value).unwrap_or_default();
        body.push(b'\n');
        Self {
            status,
            reason,
            body,
        }
    }

    /// Returns the HTTP status code.
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Returns the response body bytes.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Writes the response as HTTP/1.1 wire bytes.
    pub fn write_to(&self, writer: &mut dyn Write) -> io::Result<()> {
        write!(
            writer,
            "HTTP/1.1 {} {}\r\n\
             content-type: application/json\r\n\
             content-length: {}\r\n\
             connection: close\r\n\
             cache-control: no-store\r\n\r\n",
            self.status,
            self.reason,
            self.body.len()
        )?;
        writer.write_all(&self.body)
    }
}

/// Routes a request method and path against the history store.
pub fn handle_request(method: &str, path: &str, store: &LocalHistoryStore) -> HttpResponse {
    if method != "GET" {
        return HttpResponse::json(
            405,
            "Method Not Allowed",
            &json!({ "error": "method_not_allowed" }),
        );
    }
    match path {
        "/api/health" => HttpResponse::json(
            200,
            "OK",
            &json!({
                "status": "ok",
                "schema_version": LOCAL_REPORT_SCHEMA_VERSION,
                "binding": "loopback"
            }),
        ),
        "/api/history" => history_index(store),
        _ => match path.strip_prefix("/api/history/") {
            Some(id) if !id.is_empty() && !id.contains('/') => history_record(store, id),
            _ => HttpResponse::json(404, "Not Found", &json!({ "error": "not_found" })),
        },
    }
}

fn history_index(store: &LocalHistoryStore) -> HttpResponse {
    match store.list_active() {
        Ok(records) => {
            let records: Vec<Value> = records
                .iter()
                .map(|record| {
                    json!({
                        "id": record.id(),
                        "sequence": record.sequence(),
                        "recorded_at": record.recorded_at(),
                        "repository_id": record.repository_id()
                    })
                })
                .collect();
            HttpResponse::json(
                200,
                "OK",
                &json!({
                    "schema_version": LOCAL_REPORT_SCHEMA_VERSION,
                    "records": records
                }),
            )
        }
        Err(_) => store_error(),
    }
}

fn history_record(store: &LocalHistoryStore, id: &str) -> HttpResponse {
    match store.get_active(id) {
        Ok(Some(record)) => HttpResponse::json(200, "OK", record.report()),
        Ok(None) => HttpResponse::json(404, "Not Found", &json!({ "error": "not_found" })),
        Err(_) => store_error(),
    }
}

fn store_error() -> HttpResponse {
    HttpResponse::json(
        500,
        "Internal Server Error",
        &json!({ "error": "history_unavailable" }),
    )
}

/// Reads one bounded request line from a connection and writes the routed
/// response. A line exceeding [`MAX_REQUEST_LINE_BYTES`] is rejected instead of
/// being buffered without limit.
pub fn serve_connection(
    reader: &mut dyn Read,
    writer: &mut dyn Write,
    store: &LocalHistoryStore,
) -> io::Result<()> {
    let mut buffered = BufReader::new(reader.take((MAX_REQUEST_LINE_BYTES + 1) as u64));
    let mut request_line = String::new();
    buffered.read_line(&mut request_line)?;
    let response = if request_line.len() > MAX_REQUEST_LINE_BYTES {
        HttpResponse::json(
            431,
            "Request Header Fields Too Large",
            &json!({ "error": "request_line_too_long" }),
        )
    } else {
        let (method, path) = parse_request_line(&request_line);
        handle_request(method, path, store)
    };
    response.write_to(writer)?;
    writer.flush()
}

/// Applies a read timeout to an accepted connection.
fn configure_stream(stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(READ_TIMEOUT))
}

fn parse_request_line(line: &str) -> (&str, &str) {
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    (method, path)
}

/// Accepts and serves the next inbound loopback connection.
pub fn serve_next(listener: &LoopbackListener, store: &LocalHistoryStore) -> io::Result<()> {
    let mut stream = listener.accept()?;
    configure_stream(&stream)?;
    let mut peer = stream.try_clone()?;
    serve_connection(&mut peer, &mut stream, store)
}

/// Serves loopback connections until an accept error occurs.
pub fn run(listener: &LoopbackListener, store: &LocalHistoryStore) -> io::Result<()> {
    loop {
        serve_next(listener, store)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn store_with_record() -> (TempDir, LocalHistoryStore) {
        let dir = TempDir::new().unwrap();
        let store = LocalHistoryStore::open(dir.path()).unwrap();
        store
            .append(
                json!({ "repository": { "repository_id": "abc" }, "visibility": "private_local" }),
                "2026-07-16T00:00:00Z",
            )
            .unwrap();
        (dir, store)
    }

    #[test]
    fn health_reports_loopback_binding() {
        let dir = TempDir::new().unwrap();
        let store = LocalHistoryStore::open(dir.path()).unwrap();
        let response = handle_request("GET", "/api/health", &store);
        assert_eq!(response.status(), 200);
        let value: Value = serde_json::from_slice(response.body()).unwrap();
        assert_eq!(value["binding"], "loopback");
    }

    #[test]
    fn index_and_record_routes_serve_active_history() {
        let (_dir, store) = store_with_record();
        let index = handle_request("GET", "/api/history", &store);
        let value: Value = serde_json::from_slice(index.body()).unwrap();
        assert_eq!(value["records"][0]["id"], "rec-000001");

        let record = handle_request("GET", "/api/history/rec-000001", &store);
        assert_eq!(record.status(), 200);
        let body: Value = serde_json::from_slice(record.body()).unwrap();
        assert_eq!(body["visibility"], "private_local");

        let missing = handle_request("GET", "/api/history/rec-999999", &store);
        assert_eq!(missing.status(), 404);
    }

    #[test]
    fn non_get_methods_are_rejected() {
        let dir = TempDir::new().unwrap();
        let store = LocalHistoryStore::open(dir.path()).unwrap();
        assert_eq!(handle_request("POST", "/api/history", &store).status(), 405);
        assert_eq!(
            handle_request("DELETE", "/api/history/rec-1", &store).status(),
            405
        );
    }

    #[test]
    fn connection_round_trip_writes_http_response() {
        let (_dir, store) = store_with_record();
        let mut request = &b"GET /api/health HTTP/1.1\r\n\r\n"[..];
        let mut output = Vec::new();
        serve_connection(&mut request, &mut output, &store).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK"));
        assert!(text.contains("\"binding\":\"loopback\""));
    }

    #[test]
    fn overlong_request_line_is_rejected_without_unbounded_read() {
        let (_dir, store) = store_with_record();
        // A stream that never sends a newline must not grow memory unbounded or
        // stall; the bounded read rejects it instead of reading everything.
        let request = vec![b'A'; MAX_REQUEST_LINE_BYTES * 4];
        let mut reader = &request[..];
        let mut output = Vec::new();
        serve_connection(&mut reader, &mut output, &store).unwrap();
        let text = String::from_utf8(output).unwrap();
        assert!(text.starts_with("HTTP/1.1 431"));
        assert!(text.contains("request_line_too_long"));
        // The bounded reader left the remaining bytes unconsumed.
        assert!(reader.len() >= request.len() - (MAX_REQUEST_LINE_BYTES + 1));
    }

    #[test]
    fn accepted_stream_carries_a_read_timeout() {
        let listener = LoopbackListener::bind(0).unwrap();
        let address = listener.local_addr().unwrap();
        let _client = std::net::TcpStream::connect(address).unwrap();
        let accepted = listener.accept().unwrap();
        configure_stream(&accepted).unwrap();
        assert_eq!(accepted.read_timeout().unwrap(), Some(READ_TIMEOUT));
    }
}
