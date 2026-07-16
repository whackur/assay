//! Proves the dashboard binds only loopback and serves a real round trip.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpStream};
use std::thread;

use assay_local::{LocalHistoryStore, LoopbackListener, serve_next};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn dashboard_round_trip_over_loopback() {
    let dir = TempDir::new().unwrap();
    let store = LocalHistoryStore::open(dir.path()).unwrap();
    store
        .append(
            json!({ "repository": { "repository_id": "abc" }, "visibility": "private_local" }),
            "2026-07-16T00:00:00Z",
        )
        .unwrap();

    let listener = LoopbackListener::bind(0).unwrap();
    let address = listener.local_addr().unwrap();
    assert!(address.ip().is_loopback());
    assert_eq!(address.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));

    let server = thread::spawn(move || {
        serve_next(&listener, &store).unwrap();
    });

    let mut client = TcpStream::connect(address).unwrap();
    client
        .write_all(b"GET /api/history/rec-000001 HTTP/1.1\r\nhost: localhost\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    server.join().unwrap();

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("\"visibility\":\"private_local\""));
    assert!(response.contains("cache-control: no-store"));
}
