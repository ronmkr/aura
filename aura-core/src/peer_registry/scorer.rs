use super::logic::PeerState;

pub trait PeerScorer: Send + Sync + std::fmt::Debug {
    fn calculate_score(&self, state: &PeerState) -> f64;
}

#[derive(Debug)]
pub struct DefaultScorer;

impl PeerScorer for DefaultScorer {
    fn calculate_score(&self, state: &PeerState) -> f64 {
        let throughput = state.download_rate + state.upload_rate;
        let error_penalty = (state.error_count as f64) * 100000.0;
        let idle_secs = state.last_activity.elapsed().as_secs_f64();
        let idle_penalty = if idle_secs > 60.0 {
            idle_secs * 100.0
        } else {
            0.0
        };
        throughput - error_penalty - idle_penalty
    }
}

/// A scorer strategy that prioritizes raw download and upload throughput,
/// with an extra bias toward download speed.
#[derive(Debug)]
pub struct ThroughputPriorityScorer;

impl PeerScorer for ThroughputPriorityScorer {
    fn calculate_score(&self, state: &PeerState) -> f64 {
        let weighted_throughput = state.download_rate * 2.0 + state.upload_rate;
        let error_penalty = (state.error_count as f64) * 200000.0;
        let idle_secs = state.last_activity.elapsed().as_secs_f64();
        let idle_penalty = if idle_secs > 30.0 {
            idle_secs * 200.0
        } else {
            0.0
        };
        weighted_throughput - error_penalty - idle_penalty
    }
}

/// A scorer strategy that penalizes peers that are snubbing us
/// (i.e. we are interested, but downloading is inactive or choked).
#[derive(Debug)]
pub struct AntiSnubbingScorer {
    pub snub_timeout_secs: f64,
}

impl Default for AntiSnubbingScorer {
    fn default() -> Self {
        Self {
            snub_timeout_secs: 60.0,
        }
    }
}

impl PeerScorer for AntiSnubbingScorer {
    fn calculate_score(&self, state: &PeerState) -> f64 {
        let base_score = state.download_rate + state.upload_rate;
        let error_penalty = (state.error_count as f64) * 100000.0;

        let idle_secs = state.last_activity.elapsed().as_secs_f64();

        // Snubbing penalty: we are interested, but download rate is negligible and we haven't seen activity
        let snubbed_penalty = if state.am_interested
            && state.download_rate < 1.0
            && idle_secs > self.snub_timeout_secs
        {
            500000.0 + idle_secs * 1000.0
        } else {
            0.0
        };

        // Choke penalty: we are interested, but they are choking us
        let choke_penalty = if state.am_interested && state.peer_choking {
            50000.0
        } else {
            0.0
        };

        base_score - error_penalty - snubbed_penalty - choke_penalty
    }
}

#[cfg(test)]
#[path = "scorer_tests.rs"]
mod tests;
