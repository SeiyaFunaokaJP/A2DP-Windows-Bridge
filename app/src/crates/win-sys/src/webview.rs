//! WebView2 reporting its own processes failing (ProcessFailed), for
//! a2dpwb.exe to recover a black window (its recovery.rs)

use webview2_com::Microsoft::Web::WebView2::Win32::{
    COREWEBVIEW2_PROCESS_FAILED_KIND, COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED,
    COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE, ICoreWebView2Controller,
};
use webview2_com::ProcessFailedEventHandler;

/// Which process failed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessFailed {
    /// The browser process: every webview of the environment is gone
    BrowserExited,
    /// The page's renderer exited
    RendererExited,
    /// The page's renderer hangs
    RendererUnresponsive,
    /// GPU, utility and the others (COREWEBVIEW2_PROCESS_FAILED_KIND):
    /// WebView2 starts them again itself
    Other(i32),
}

impl From<COREWEBVIEW2_PROCESS_FAILED_KIND> for ProcessFailed {
    fn from(kind: COREWEBVIEW2_PROCESS_FAILED_KIND) -> Self {
        match kind {
            COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED => ProcessFailed::BrowserExited,
            COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED => ProcessFailed::RendererExited,
            COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE => ProcessFailed::RendererUnresponsive,
            other => ProcessFailed::Other(other.0),
        }
    }
}

/// Calls `f` whenever a process of the controller's webview fails. Call on
/// the thread that owns the controller (Tauri's with_webview runs there);
/// the handler stays for the webview's life.
pub fn on_process_failed(
    controller: &ICoreWebView2Controller,
    mut f: impl FnMut(ProcessFailed) + 'static,
) -> windows::core::Result<()> {
    let handler = ProcessFailedEventHandler::create(Box::new(move |_, args| {
        let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND::default();
        if let Some(args) = args {
            // SAFETY: a COM getter writing into a local
            unsafe { args.ProcessFailedKind(&mut kind)? };
        }
        f(kind.into());
        Ok(())
    }));
    let mut token = 0;
    // SAFETY: COM calls on the caller's controller, on its own thread; the
    // token is not kept (the handler is never removed)
    unsafe { controller.CoreWebView2()?.add_ProcessFailed(&handler, &mut token) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED;

    #[test]
    fn kinds_map_to_what_recovery_acts_on() {
        assert_eq!(
            ProcessFailed::from(COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED),
            ProcessFailed::BrowserExited
        );
        assert_eq!(
            ProcessFailed::from(COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED),
            ProcessFailed::RendererExited
        );
        assert_eq!(
            ProcessFailed::from(COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE),
            ProcessFailed::RendererUnresponsive
        );
        assert!(matches!(
            ProcessFailed::from(COREWEBVIEW2_PROCESS_FAILED_KIND_GPU_PROCESS_EXITED),
            ProcessFailed::Other(_)
        ));
    }
}
