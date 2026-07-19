//! `serve --once` HTTP helper for the cross-component integration tests.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};

use super::common::{FIXED_TIME, binary};

// Spawns `serve --once`, issues one GET, and returns the raw HTTP response.
pub(crate) fn serve_once_get(history: &std::path::Path, path: &str) -> String {
    let mut command = Command::new(binary());
    command.env_clear().env("ASSAY_TEST_FIXED_TIME", FIXED_TIME);
    // Windows sockets fail to initialize without `SystemRoot`, so preserve it
    // after clearing the environment. It carries no repository-derived input.
    #[cfg(windows)]
    if let Some(root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", root);
    }
    let mut child = command
        .arg("serve")
        .arg("--history")
        .arg(history)
        .args(["--port", "0", "--once"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("serve subprocess must start");
    let stderr = child.stderr.take().expect("serve stderr");
    let mut reader = BufReader::new(stderr);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("serve announces address");
    let address = line
        .trim()
        .rsplit("http://")
        .next()
        .expect("address token")
        .to_owned();
    let mut client = TcpStream::connect(&address).expect("connect loopback");
    client
        .write_all(format!("GET {path} HTTP/1.1\r\nhost: localhost\r\n\r\n").as_bytes())
        .unwrap();
    let mut response = String::new();
    client.read_to_string(&mut response).unwrap();
    child.wait().expect("serve exits after one request");
    response
}
