//! `bluecaster-core.exe --quit`: asks the running core of this user to exit
//! (app.quit over its pipe, as the UI does) and waits until it has. The
//! installer and the uninstaller run it before they replace or remove the
//! program's files, so that streaming stops and the adapter is released
//! cleanly instead of the process being killed.
//!
//! Exit code 0 when a core ran and has exited, 2 when none ran, 1 when it
//! still runs after the wait (the installer then ends the process itself).

use std::time::{Duration, Instant};

use ipc::{ClientMessage, Method, PROTOCOL_VERSION};
use serde_json::Value;
use tokio::net::windows::named_pipe::ClientOptions;

/// All pipe instances busy: try again shortly
const ERROR_PIPE_BUSY: i32 = 231;
/// Connecting and the request
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// Stopping a stream and resetting the adapter takes a few seconds
const EXIT_TIMEOUT: Duration = Duration::from_secs(15);

fn core_running() -> bool {
    win_sys::single_instance::is_held(&ipc::core_mutex_name())
}

async fn request_quit(name: &str) -> std::io::Result<()> {
    let pipe = loop {
        match ClientOptions::new().open(name) {
            Ok(p) => break p,
            Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Err(e) => return Err(e),
        }
    };
    let (mut rd, mut wr) = tokio::io::split(pipe);
    let io = |e: ipc::FrameError| std::io::Error::other(e.to_string());
    let hello = ClientMessage::Hello {
        protocol: PROTOCOL_VERSION,
        client: format!("bluecaster-quit/{}", env!("CARGO_PKG_VERSION")),
    };
    ipc::write_frame(&mut wr, &hello).await.map_err(io)?;
    ipc::write_frame(&mut wr, &ClientMessage::Request { id: 1, call: Method::AppQuit }).await.map_err(io)?;
    // Until the answer, Bye or the pipe closing
    loop {
        let msg: Option<Value> = ipc::read_frame(&mut rd).await.map_err(io)?;
        let Some(msg) = msg else { return Ok(()) };
        match msg.get("type").and_then(Value::as_str) {
            Some("reject") => {
                let reason = msg.get("reason").and_then(Value::as_str).unwrap_or("");
                return Err(std::io::Error::other(format!("rejected: {reason}")));
            }
            Some("response") if msg.get("id").and_then(Value::as_u64) == Some(1) => return Ok(()),
            _ if msg.pointer("/event/kind").and_then(Value::as_str) == Some("bye") => return Ok(()),
            _ => {}
        }
    }
}

pub fn run() -> i32 {
    if !core_running() {
        return 2;
    }
    let sid = match win_sys::pipe::current_user_sid() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bluecaster-core --quit: user SID: {e}");
            return 1;
        }
    };
    let name = ipc::pipe_name(&sid);
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("bluecaster-core --quit: {e}");
            return 1;
        }
    };
    match rt.block_on(async { tokio::time::timeout(REQUEST_TIMEOUT, request_quit(&name)).await }) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => eprintln!("bluecaster-core --quit: {e}"),
        Err(_) => eprintln!("bluecaster-core --quit: no answer"),
    }
    let start = Instant::now();
    while core_running() {
        if start.elapsed() >= EXIT_TIMEOUT {
            eprintln!("bluecaster-core --quit: the core still runs");
            return 1;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    0
}
