//! `bluecaster-core.exe --quit`: asks the running core of this user to exit
//! (app.quit over its pipe, as the UI does) and waits until it has. The
//! installer and the uninstaller run it before they replace or remove the
//! program's files, so that streaming stops and the adapter is released
//! cleanly instead of the process being killed.
//!
//! A 2.0.x core (A2DPWB, its own mutex and only the old pipe name) is
//! stopped the same way: the new pipe name is tried first, then the old
//! one, and the wait is for both mutexes. The resident start uses
//! [`stop_old_core`] to replace a 2.0.x core that still runs.
//!
//! Exit code 0 when a core ran and has exited, 2 when none ran, 1 when it
//! still runs after the wait (the installer then ends the process itself).

use std::time::{Duration, Instant};

use ipc::{ClientMessage, Method, PROTOCOL_VERSION};
use serde_json::Value;
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

/// All pipe instances busy: try again shortly
const ERROR_PIPE_BUSY: i32 = 231;
/// Connecting and the request
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
/// Stopping a stream and resetting the adapter takes a few seconds
const EXIT_TIMEOUT: Duration = Duration::from_secs(15);

/// A core of this user runs: a new one, or a 2.0.x one
fn core_running() -> bool {
    use win_sys::single_instance::is_held;
    is_held(&ipc::core_mutex_name()) || old_core_running()
}

/// A 2.0.x core runs (or a new one holding its mutex, see resident.rs)
pub fn old_core_running() -> bool {
    win_sys::single_instance::is_held(&ipc::old_core_mutex_name())
}

/// Opens the first of `names` that exists
async fn open_first(names: &[String]) -> std::io::Result<NamedPipeClient> {
    let mut last = std::io::Error::from(std::io::ErrorKind::NotFound);
    for name in names {
        loop {
            match ClientOptions::new().open(name) {
                Ok(p) => return Ok(p),
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                // Not served under this name: the next one
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    last = e;
                    break;
                }
                Err(e) => return Err(e),
            }
        }
    }
    Err(last)
}

/// Sends app.quit over the first of `names` that exists
async fn request_quit(names: &[String]) -> std::io::Result<()> {
    let pipe = open_first(names).await?;
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
    let names = [ipc::pipe_name(&sid), ipc::old_pipe_name(&sid)];
    let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("bluecaster-core --quit: {e}");
            return 1;
        }
    };
    match rt.block_on(async { tokio::time::timeout(REQUEST_TIMEOUT, request_quit(&names)).await }) {
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

/// Replaces a 2.0.x core that still runs (its mutex is held): asks it to
/// exit over the old pipe name, as `--quit` does, and waits up to `wait`
/// for its mutex to go. True if no old core runs any more.
pub fn stop_old_core(wait: Duration) -> bool {
    if !old_core_running() {
        return true;
    }
    let names = match win_sys::pipe::current_user_sid() {
        Ok(sid) => [ipc::old_pipe_name(&sid)],
        Err(e) => {
            tracing::warn!("2.0.x core: user SID: {e}");
            return false;
        }
    };
    tracing::info!("a 2.0.x core runs: asking it to exit over {}", names[0]);
    let asked = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(rt) => rt.block_on(async { tokio::time::timeout(REQUEST_TIMEOUT, request_quit(&names)).await }),
        Err(e) => Ok(Err(std::io::Error::other(e))),
    };
    match asked {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!("2.0.x core: app.quit: {e}"),
        Err(_) => tracing::warn!("2.0.x core: no answer to app.quit"),
    }
    let start = Instant::now();
    while old_core_running() {
        if start.elapsed() >= wait {
            tracing::warn!("the 2.0.x core still runs after {wait:?}: it may hold the Bluetooth adapter");
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    tracing::info!("the 2.0.x core has exited after {} ms", start.elapsed().as_millis());
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The new name first, then the old one (a 2.0.x core serves only that)
    #[test]
    fn falls_back_to_the_old_pipe_name() {
        let pid = std::process::id();
        let new = format!(r"\\.\pipe\bluecaster-test-quit-{pid}");
        let old = format!(r"\\.\pipe\a2dpwb-test-quit-{pid}");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let names = [new.clone(), old.clone()];
            let e = open_first(&names).await.unwrap_err();
            assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
            let _old_server = win_sys::pipe::create_server(&old, true).unwrap();
            open_first(&names).await.unwrap();
            let _new_server = win_sys::pipe::create_server(&new, true).unwrap();
            open_first(&names).await.unwrap();
        });
    }
}
