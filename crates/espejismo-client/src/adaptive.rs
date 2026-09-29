//! Adaptive throughput: BDP-based buffer/window floors driven by measured RTT.
//!
//! RTT is sampled on every lane connect from TCP connect time and handshake
//! time. Both are mux-agnostic, so Yamux (which has no mux-level ping) gets a
//! usable RTT source without vendored changes.
//!
//! The mux window is recomputed without an RTT gate from every smoothed RTT
//! sample, while tunnel and TCP buffer boosts retain a two-threshold
//! hysteresis gate. Only tunables the operator left at their defaults are
//! raised, and only by lifting them to the BDP floor. New values apply to
//! subsequently established sessions; already connected sessions keep
//! draining with the parameters they were created with.

use std::time::Duration;

use espejismo_core::{adaptive_throughput_floor, AdaptiveEligibility, MuxRuntimeConfig, TcpConfig};

/// EWMA smoothing factor for RTT samples.
const RTT_ALPHA: f64 = 0.3;
/// Engage the boost when the smoothed RTT stays above this.
const ENGAGE_RTT_MS: f64 = 150.0;
/// Release the boost when the smoothed RTT stays below this.
const RELEASE_RTT_MS: f64 = 80.0;
/// Consecutive samples above the engage threshold needed to engage.
const ENGAGE_SAMPLES: u32 = 2;
/// Consecutive samples below the release threshold needed to release.
const RELEASE_SAMPLES: u32 = 3;

#[derive(Debug)]
pub(crate) struct AdaptiveThroughput {
    baseline_mux_window: usize,
    baseline_tunnel_buffer: usize,
    baseline_tcp_send: usize,
    baseline_tcp_recv: usize,
    eligibility: AdaptiveEligibility,
    smoothed_rtt_ms: f64,
    initialized: bool,
    above_engage: u32,
    below_release: u32,
    boost_active: bool,
    mux: MuxRuntimeConfig,
    tunnel_buffer: usize,
    tcp: TcpConfig,
}

impl AdaptiveThroughput {
    pub(crate) fn new(
        mux: MuxRuntimeConfig,
        tunnel_buffer: usize,
        tcp: TcpConfig,
        eligibility: AdaptiveEligibility,
    ) -> Self {
        Self {
            baseline_mux_window: mux.native_initial_window_bytes,
            baseline_tunnel_buffer: tunnel_buffer,
            baseline_tcp_send: tcp.send_buffer_bytes,
            baseline_tcp_recv: tcp.recv_buffer_bytes,
            eligibility,
            smoothed_rtt_ms: 0.0,
            initialized: false,
            above_engage: 0,
            below_release: 0,
            boost_active: false,
            mux,
            tunnel_buffer,
            tcp,
        }
    }

    /// Feed one RTT sample (TCP connect time or handshake time).
    pub(crate) fn observe_rtt(&mut self, rtt: Duration) {
        let ms = rtt.as_secs_f64() * 1000.0;
        if !self.initialized {
            self.smoothed_rtt_ms = ms;
            self.initialized = true;
        } else {
            self.smoothed_rtt_ms = RTT_ALPHA * ms + (1.0 - RTT_ALPHA) * self.smoothed_rtt_ms;
        }

        if self.eligibility.mux_window {
            let floor =
                adaptive_throughput_floor(Duration::from_secs_f64(self.smoothed_rtt_ms / 1000.0));
            self.mux.native_initial_window_bytes =
                self.baseline_mux_window.max(floor.mux_window_bytes);
        }

        if self.smoothed_rtt_ms > ENGAGE_RTT_MS {
            self.above_engage = self.above_engage.saturating_add(1);
            self.below_release = 0;
        } else if self.smoothed_rtt_ms < RELEASE_RTT_MS {
            self.below_release = self.below_release.saturating_add(1);
            self.above_engage = 0;
        }
        // Inside the deadband the counters are kept, so a single mid-range
        // sample neither engages the boost nor cancels a trend.

        if !self.boost_active && self.above_engage >= ENGAGE_SAMPLES {
            self.engage();
        } else if self.boost_active && self.below_release >= RELEASE_SAMPLES {
            self.release();
        }
    }

    fn engage(&mut self) {
        let floor =
            adaptive_throughput_floor(Duration::from_secs_f64(self.smoothed_rtt_ms / 1000.0));
        if self.eligibility.tunnel_buffer {
            self.tunnel_buffer = self.tunnel_buffer.max(floor.tunnel_buffer);
        }
        if self.eligibility.tcp_buffers {
            self.tcp.send_buffer_bytes = self.tcp.send_buffer_bytes.max(floor.tcp_buffer_bytes);
            self.tcp.recv_buffer_bytes = self.tcp.recv_buffer_bytes.max(floor.tcp_buffer_bytes);
        }
        self.boost_active = true;
        self.above_engage = 0;
        tracing::info!(
            smoothed_rtt_ms = self.smoothed_rtt_ms.round() as u64,
            mux_window_bytes = self.mux.native_initial_window_bytes,
            tunnel_buffer = self.tunnel_buffer,
            "adaptive throughput engaged for high-RTT path",
        );
    }

    fn release(&mut self) {
        self.tunnel_buffer = self.baseline_tunnel_buffer;
        if self.eligibility.tcp_buffers {
            self.tcp.send_buffer_bytes = self.baseline_tcp_send;
            self.tcp.recv_buffer_bytes = self.baseline_tcp_recv;
        }
        self.boost_active = false;
        self.below_release = 0;
        tracing::info!(
            smoothed_rtt_ms = self.smoothed_rtt_ms.round() as u64,
            "adaptive throughput released, restored baseline values",
        );
    }

    pub(crate) fn mux(&self) -> MuxRuntimeConfig {
        self.mux
    }

    pub(crate) fn tunnel_buffer(&self) -> usize {
        self.tunnel_buffer
    }

    pub(crate) fn tcp(&self) -> TcpConfig {
        self.tcp.clone()
    }

    #[cfg(test)]
    fn boost_active(&self) -> bool {
        self.boost_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use espejismo_core::AdaptiveEligibility;

    fn test_state() -> AdaptiveThroughput {
        let mux = MuxRuntimeConfig {
            mode: espejismo_core::config::MuxMode::Yamux,
            max_streams: 256,
            native_initial_window_bytes: 8 * 1024 * 1024,
            native_stream_buffer_frames: 128,
            native_send_queue_frames: 64,
            native_idle_timeout: Duration::from_secs(300),
            native_drain_timeout: Duration::from_secs(30),
        };
        let eligibility = AdaptiveEligibility::from_tunables(1024 * 1024, 8 * 1024 * 1024, 0, 0);
        assert!(eligibility.tunnel_buffer && eligibility.mux_window && eligibility.tcp_buffers);
        AdaptiveThroughput::new(mux, 1024 * 1024, TcpConfig::default(), eligibility)
    }

    #[test]
    fn low_rtt_raises_window_without_engaging_buffer_boost() {
        let mut st = test_state();
        st.observe_rtt(Duration::from_millis(200));
        assert!(!st.boost_active());
        assert_eq!(st.tunnel_buffer(), 1024 * 1024);
        assert_eq!(st.mux().native_initial_window_bytes, 25_000_000);

        let mut clamped = test_state();
        clamped.observe_rtt(Duration::from_secs(10));
        assert_eq!(clamped.mux().native_initial_window_bytes, 64 * 1024 * 1024);
    }

    #[test]
    fn sustained_high_rtt_engages_and_raises_floors() {
        let mut st = test_state();
        // One high sample is not enough (transient spike protection).
        st.observe_rtt(Duration::from_millis(300));
        assert!(!st.boost_active());
        // Second consecutive high sample engages.
        st.observe_rtt(Duration::from_millis(300));
        assert!(st.boost_active());
        // BDP at ~300 ms smoothed: well above the 1 MiB baseline.
        assert!(st.tunnel_buffer() > 1024 * 1024);
        assert!(st.mux().native_initial_window_bytes > 8 * 1024 * 1024);
        assert_eq!(st.tcp().send_buffer_bytes, 4 * 1024 * 1024);
    }

    #[test]
    fn deadband_does_not_flap() {
        let mut st = test_state();
        st.observe_rtt(Duration::from_millis(300));
        st.observe_rtt(Duration::from_millis(300));
        assert!(st.boost_active());
        // Mid-range samples keep the boost; they don't release it.
        for _ in 0..10 {
            st.observe_rtt(Duration::from_millis(100));
        }
        assert!(st.boost_active());
    }

    #[test]
    fn sustained_low_rtt_releases_to_baseline() {
        let mut st = test_state();
        st.observe_rtt(Duration::from_millis(300));
        st.observe_rtt(Duration::from_millis(300));
        assert!(st.boost_active());
        // EWMA decays geometrically, so the smoothed RTT needs several low
        // samples to fall into the release zone; a brief dip must not release.
        for _ in 0..5 {
            st.observe_rtt(Duration::from_millis(20));
            assert!(st.boost_active());
        }
        // Once the smoothed RTT is in the release zone, three consecutive low
        // samples release the boost.
        for _ in 0..3 {
            st.observe_rtt(Duration::from_millis(20));
        }
        assert!(!st.boost_active());
        assert_eq!(st.tunnel_buffer(), 1024 * 1024);
        assert!(st.mux().native_initial_window_bytes >= 8 * 1024 * 1024);
        assert_eq!(st.tcp().send_buffer_bytes, 0);
    }

    #[test]
    fn explicit_config_is_not_touched() {
        let mux = MuxRuntimeConfig {
            mode: espejismo_core::config::MuxMode::Yamux,
            max_streams: 256,
            native_initial_window_bytes: 2 * 1024 * 1024,
            native_stream_buffer_frames: 128,
            native_send_queue_frames: 64,
            native_idle_timeout: Duration::from_secs(300),
            native_drain_timeout: Duration::from_secs(30),
        };
        // Operator explicitly set window and tcp buffers; only tunnel_buffer
        // (still default) is eligible.
        let eligibility = AdaptiveEligibility::from_tunables(
            1024 * 1024,
            2 * 1024 * 1024,
            512 * 1024,
            512 * 1024,
        );
        assert!(eligibility.tunnel_buffer);
        assert!(!eligibility.mux_window);
        assert!(!eligibility.tcp_buffers);
        let tcp = TcpConfig {
            send_buffer_bytes: 512 * 1024,
            recv_buffer_bytes: 512 * 1024,
            ..Default::default()
        };
        let mut st = AdaptiveThroughput::new(mux, 1024 * 1024, tcp, eligibility);
        st.observe_rtt(Duration::from_millis(300));
        st.observe_rtt(Duration::from_millis(300));
        assert!(st.boost_active());
        assert!(st.tunnel_buffer() > 1024 * 1024);
        assert_eq!(st.mux().native_initial_window_bytes, 2 * 1024 * 1024);
        assert_eq!(st.tcp().send_buffer_bytes, 512 * 1024);
    }

    #[test]
    fn falling_rtt_never_lowers_window_below_baseline() {
        let mut st = test_state();
        for rtt in [500, 300, 200, 100, 50, 20, 10] {
            st.observe_rtt(Duration::from_millis(rtt));
            assert!(st.mux().native_initial_window_bytes >= 8 * 1024 * 1024);
        }
    }
}
