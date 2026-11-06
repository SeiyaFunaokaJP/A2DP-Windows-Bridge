//! What SBC's and aptX Adaptive's ABR remember of a stall (#279): a sink
//! that stalls at one step runs the step below fine, and climbing back the
//! usual way (a few seconds) makes it stall again and again

/// ABR steps (100 ms each) of a drained queue before going back up to the
/// step the queue backed up at: 10 s for the first stall there, doubled for
/// each stall at or below it ...
pub const STALL_TICKS: u32 = 100;
/// ... up to 2 minutes
pub const STALL_MAX_TICKS: u32 = 1200;
/// Steps without a stall at or above that step after which ABR forgets it:
/// 1 minute
pub const FORGET_TICKS: u32 = 600;

/// The stall memory of one ABR. Steps are numbers that grow with the bit
/// rate (the SBC bitpool, the aptX Adaptive level).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StallMemory {
    /// The steps down since the queue last stayed drained for the usual
    /// wait to step up are one stall
    in_stall: bool,
    stall: Option<Stall>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Stall {
    /// The step the stall began at
    at: u8,
    /// Drained steps needed to go back up to it
    wait: u32,
    /// Steps run at or above it since, without a stall
    held: u32,
}

impl StallMemory {
    /// ABR steps down from `from`: the first step down of a stall notes
    /// where it began. A new stall there or below waits twice as long as
    /// the last before going back up.
    pub fn stepped_down(&mut self, codec: &str, from: u8) {
        if std::mem::replace(&mut self.in_stall, true) {
            return;
        }
        let wait = match self.stall {
            Some(s) if from <= s.at => (s.wait * 2).min(STALL_MAX_TICKS),
            _ => STALL_TICKS,
        };
        tracing::debug!("{codec}: ABR stall at {from}, {} s of a drained queue before going back to it", wait / 10);
        self.stall = Some(Stall { at: from, wait, held: 0 });
    }

    /// One ABR step without a step down, at step `now`, the queue drained
    /// for `calm` steps; `usual` is the wait for one step up
    pub fn tick(&mut self, codec: &str, now: u8, calm: u32, usual: u32) {
        if calm >= usual {
            self.in_stall = false;
        }
        if let Some(s) = self.stall.as_mut()
            && now >= s.at
        {
            s.held += 1;
            if s.held >= FORGET_TICKS {
                tracing::debug!("{codec}: ABR held {} for a minute, stall forgotten", s.at);
                self.stall = None;
            }
        }
    }

    /// Drained steps needed to go up from `now` to `next`: `usual`, or the
    /// stall's wait when `next` reaches the step it began at
    pub fn wait(&self, now: u8, next: u8, usual: u32) -> u32 {
        match self.stall {
            Some(s) if now < s.at && next >= s.at => s.wait.max(usual),
            _ => usual,
        }
    }
}
