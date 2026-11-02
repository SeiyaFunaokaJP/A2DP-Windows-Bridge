//! The RFCOMM channel to earbuds' own control service, over the link the stack
//! has to them. Closed when dropped.

use std::sync::mpsc;
use std::time::Instant;

use bt_host::{BtHost, RfcommError, RfcommEvent, RfcommOpened, RfcommSink};

/// Where a control service is looked for, tried in order
pub(super) enum Place {
    Service(&'static str),
    Channel(u8),
}

/// What came of waiting for data
pub(super) enum Received {
    Data(Vec<u8>),
    Timeout,
    /// The earbuds went away, or restart
    Closed,
}

pub(super) struct Channel<'a> {
    host: &'a BtHost,
    id: u32,
    events: mpsc::Receiver<RfcommEvent>,
    /// For the log
    vendor: &'static str,
    closed: bool,
}

impl<'a> Channel<'a> {
    pub(super) fn open(host: &'a BtHost, addr: [u8; 6], vendor: &'static str, places: &[Place]) -> Option<Channel<'a>> {
        let mut opened = Err(RfcommError::ServiceNotFound);
        for place in places {
            if !matches!(opened, Err(RfcommError::ServiceNotFound | RfcommError::Refused(_))) {
                break;
            }
            opened = match *place {
                Place::Service(uuid) => {
                    let Some(uuid) = bt_host::parse_uuid(uuid) else { continue };
                    Channel::connect(host, vendor, |sink| host.rfcomm_open(addr, uuid, sink))
                }
                Place::Channel(channel) => {
                    Channel::connect(host, vendor, |sink| host.rfcomm_open_channel(addr, channel, sink))
                }
            };
        }
        opened
            .inspect_err(|e| {
                tracing::info!("{vendor}: no control channel ({e:?}), the earbuds' mode stays as it is");
            })
            .ok()
    }

    fn connect(
        host: &'a BtHost,
        vendor: &'static str,
        open: impl FnOnce(RfcommSink) -> Result<RfcommOpened, RfcommError>,
    ) -> Result<Channel<'a>, RfcommError> {
        let (tx, events) = mpsc::channel();
        let opened = open(Box::new(move |_, event| tx.send(event).is_ok()))?;
        Ok(Channel { host, id: opened.id, events, vendor, closed: false })
    }

    /// False: not sent, and the channel counts as closed
    pub(super) fn send(&mut self, data: Vec<u8>) -> bool {
        let sent = self.host.rfcomm_send(self.id, data).wait().is_ok();
        self.closed |= !sent;
        sent
    }

    pub(super) fn receive(&mut self, deadline: Instant) -> Received {
        let left = deadline.saturating_duration_since(Instant::now());
        match self.events.recv_timeout(left) {
            Ok(RfcommEvent::Data(data)) => Received::Data(data),
            Ok(RfcommEvent::Closed(reason)) => {
                tracing::info!("{}: control channel closed ({reason:?})", self.vendor);
                self.closed = true;
                Received::Closed
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Received::Timeout,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                self.closed = true;
                Received::Closed
            }
        }
    }
}

impl Drop for Channel<'_> {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.host.rfcomm_close(self.id).wait();
        }
    }
}
