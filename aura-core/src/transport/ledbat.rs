//! LEDBAT (Low Extra Delay Background Transport, RFC 6817) Congestion Control.
//!
//! LEDBAT measures queueing delay (current delay - base delay) and adjusts the
//! congestion window to maintain the queueing delay below a target threshold (typically 100ms).

use std::time::{Duration, Instant};

/// The target queueing delay in microseconds (100ms per BEP 29 / RFC 6817).
const TARGET_DELAY_US: u64 = 100_000;

/// Base delay history tracking window duration (2 minutes).
const BASE_DELAY_WINDOW: Duration = Duration::from_secs(120);

/// Minimum congestion window in bytes (typically 2 * MSS, where MSS is 1500).
const MIN_CWND: u32 = 3000;

/// Maximum congestion window in bytes.
const MAX_CWND: u32 = 1024 * 1024 * 4; // 4MB

/// Maximum segment size in bytes.
const MSS: u32 = 1400;

/// Represents a measured base delay sample with its acquisition timestamp.
#[derive(Debug, Clone, Copy)]
struct DelaySample {
    timestamp: Instant,
    delay_us: u64,
}

/// The LEDBAT Congestion Controller.
#[derive(Debug, Clone)]
pub struct LedbatController {
    /// The current congestion window in bytes.
    cwnd: f64,
    /// Minimum observed delays to calculate the base delay.
    base_delays: Vec<DelaySample>,
    /// Last base delay value used.
    last_base_delay_us: u64,
}

impl Default for LedbatController {
    fn default() -> Self {
        Self::new()
    }
}

impl LedbatController {
    /// Creates a new LEDBAT congestion controller.
    pub fn new() -> Self {
        Self {
            cwnd: MIN_CWND as f64,
            base_delays: Vec::new(),
            last_base_delay_us: u64::MAX,
        }
    }

    /// Returns the current congestion window size in bytes.
    pub fn cwnd(&self) -> u32 {
        self.cwnd as u32
    }

    /// Records a new one-way delay measurement and updates base delay history.
    pub fn add_delay_sample(&mut self, delay_us: u64, now: Instant) {
        // Purge samples older than the 2-minute window
        self.base_delays
            .retain(|sample| now.duration_since(sample.timestamp) < BASE_DELAY_WINDOW);

        // Keep track of the minimum delay in the current window
        self.base_delays.push(DelaySample {
            timestamp: now,
            delay_us,
        });

        // Resolve base delay as the minimum observed delay
        self.last_base_delay_us = self
            .base_delays
            .iter()
            .map(|s| s.delay_us)
            .min()
            .unwrap_or(delay_us);
    }

    /// Updates the congestion window (cwnd) on receiving a packet ACK.
    ///
    /// `current_delay_us` is the delay measured for the acked packet.
    /// `bytes_newly_acked` is the size of the payload being acknowledged.
    pub fn on_ack(&mut self, current_delay_us: u64, bytes_newly_acked: u32, now: Instant) {
        self.add_delay_sample(current_delay_us, now);

        if self.last_base_delay_us == u64::MAX {
            return;
        }

        // Calculate queueing delay
        let queuing_delay = current_delay_us.saturating_sub(self.last_base_delay_us);

        // off_target indicates how far we are from the target queue delay
        let off_target = (TARGET_DELAY_US as i64) - (queuing_delay as i64);

        // delay_factor scales the adjustment relative to the target
        let delay_factor = (off_target as f64) / (TARGET_DELAY_US as f64);

        // Apply gain factor and newly acked proportion to scale cwnd adjustment
        // cwnd_increase = GAIN * delay_factor * (bytes_newly_acked / cwnd) * MSS
        // GAIN is typically 1 (meaning we scale linearly)
        let adjustment = delay_factor * (bytes_newly_acked as f64) * (MSS as f64) / self.cwnd;

        let new_cwnd = self.cwnd + adjustment;

        // Cap cwnd between minimum and maximum limits
        self.cwnd = if new_cwnd < MIN_CWND as f64 {
            MIN_CWND as f64
        } else if new_cwnd > MAX_CWND as f64 {
            MAX_CWND as f64
        } else {
            new_cwnd
        };
    }

    /// Reduces congestion window on packet loss (standard TCP back-off).
    pub fn on_loss(&mut self) {
        // Multiplicative decrease: halve the congestion window on loss, but keep it above the minimum
        self.cwnd = (self.cwnd / 2.0).max(MIN_CWND as f64);
    }
}
