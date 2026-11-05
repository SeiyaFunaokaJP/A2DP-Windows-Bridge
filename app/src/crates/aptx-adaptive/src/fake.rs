//! A stand-in codec core for testing the packet loop.

use std::collections::HashMap;

use crate::stream::table_row;
use crate::{Config, Core, Progress, Result, Row, Stream, WINDOW_SLACK};

/// Takes input until it holds one packet's worth, then waits for a flush, which is how the
/// reference behaves in steady state (spec §6). A packet is the low bytes of its first two
/// input samples, then zeros, at the table packet size.
#[derive(Debug)]
pub struct Fake {
    capacity: usize,
    buffered: usize,
    first: [i32; 2],
    /// Packet index → buffered samples at which that packet comes out without a flush.
    pub early: HashMap<usize, usize>,
    pub forced: Vec<bool>,
    pub bitrates: Vec<u32>,
    pub resets: usize,
}

/// # Errors
/// As for [`table_row`].
pub fn stream(config: Config, budget: usize) -> Result<Stream<Fake>> {
    let row = table_row(config, budget)?;
    Ok(Stream::with_core(Fake::new(config, row), config, row).with_budget(budget))
}

impl Fake {
    #[must_use]
    pub fn new(config: Config, row: Row) -> Self {
        Self {
            capacity: row.interleaved_samples(config.sample_rate, config.channels.into()),
            buffered: 0,
            first: [0; 2],
            early: HashMap::new(),
            forced: Vec::new(),
            bitrates: Vec::new(),
            resets: 0,
        }
    }

    fn packet(&mut self, window: &mut [u8], forced: bool) -> usize {
        let size = window.len() - WINDOW_SLACK;
        window[..size].fill(0);
        window[0] = self.first[0] as u8;
        window[1] = self.first[1] as u8;
        self.buffered = 0;
        self.forced.push(forced);
        size
    }
}

impl Core for Fake {
    fn encode_packet(&mut self, pcm: &[i32], window: &mut [u8]) -> Result<Progress> {
        if self.buffered == 0
            && let [a, b, ..] = *pcm
        {
            self.first = [a, b];
        }
        let early = self.early.get(&self.forced.len()).copied();
        let consumed = pcm
            .len()
            .min(early.unwrap_or(self.capacity) - self.buffered);
        self.buffered += consumed;
        let written = if early == Some(self.buffered) {
            self.packet(window, false)
        } else {
            0
        };
        Ok(Progress { consumed, written })
    }

    fn flush(&mut self, window: &mut [u8]) -> Result<usize> {
        Ok(self.packet(window, true))
    }

    fn set_bitrate(&mut self, bps: u32) {
        self.bitrates.push(bps);
    }

    fn period_changed(&mut self, samples: usize) {
        self.capacity = samples;
    }

    fn reset(&mut self, config: Config, row: &Row) -> Result<()> {
        self.capacity = row.interleaved_samples(config.sample_rate, config.channels.into());
        self.buffered = 0;
        self.resets += 1;
        Ok(())
    }
}
