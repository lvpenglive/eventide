//! Resilient Redis SUBSCRIBE loop for WAN / NAT-friendly long connections.
//!
//! - Read timeout probes keep the socket from looking eternally idle to us
//! - Idle refresh reconnects before middleboxes kill the TCP path
//! - Real disconnects reconnect with exponential backoff
//! - WARN logs are rate-limited so flaky links don't flood the console

use redis::Client;
use std::time::{Duration, Instant};

const READ_TIMEOUT: Duration = Duration::from_secs(15);
/// ~90s of silence → refresh subscription (helps cloud / NAT idle drops).
const IDLE_REFRESH_TICKS: u32 = 6;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const WARN_EVERY: Duration = Duration::from_secs(45);

/// Blocking subscribe loop. Call from a dedicated OS thread.
///
/// `on_ready` runs after each successful (re)subscribe — use it to reload
/// state that may have been missed while disconnected.
/// `on_msg` receives the channel name for each published message.
pub fn run_resilient_pubsub(
    client: &Client,
    channels: &[&str],
    mut on_ready: impl FnMut(),
    mut on_msg: impl FnMut(&str),
) -> ! {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);
    let mut last_warn = Instant::now()
        .checked_sub(WARN_EVERY)
        .unwrap_or_else(Instant::now);

    let mut warn = |msg: &str, err: &dyn std::fmt::Display| {
        let now = Instant::now();
        if now.duration_since(last_warn) >= WARN_EVERY {
            tracing::warn!(error = %err, "{msg}");
            last_warn = now;
        } else {
            tracing::debug!(error = %err, "{msg} (suppressed)");
        }
    };

    loop {
        match client.get_connection_with_timeout(CONNECT_TIMEOUT) {
            Ok(mut conn) => {
                let _ = conn.set_write_timeout(Some(CONNECT_TIMEOUT));
                let mut pubsub = conn.as_pubsub();
                if let Err(e) = pubsub.set_read_timeout(Some(READ_TIMEOUT)) {
                    warn("redis pubsub set_read_timeout failed", &e);
                }

                let mut sub_ok = true;
                for ch in channels {
                    if let Err(e) = pubsub.subscribe(*ch) {
                        warn("redis pubsub subscribe failed; retry", &e);
                        sub_ok = false;
                        break;
                    }
                }
                if !sub_ok {
                    std::thread::sleep(backoff);
                    backoff = (backoff * 2).min(max_backoff);
                    continue;
                }

                tracing::info!(?channels, "redis pubsub subscribed");
                backoff = Duration::from_secs(1);
                on_ready();

                let mut idle_ticks = 0u32;
                loop {
                    match pubsub.get_message() {
                        Ok(msg) => {
                            idle_ticks = 0;
                            on_msg(msg.get_channel_name());
                        }
                        Err(e) if e.is_timeout() => {
                            idle_ticks += 1;
                            if idle_ticks >= IDLE_REFRESH_TICKS {
                                tracing::debug!(
                                    ticks = idle_ticks,
                                    "redis pubsub idle refresh (keep NAT/firewall path warm)"
                                );
                                break;
                            }
                        }
                        Err(e) => {
                            if e.is_connection_dropped() {
                                warn("redis pubsub disconnected; reconnecting", &e);
                            } else {
                                warn("redis pubsub read failed; reconnecting", &e);
                            }
                            break;
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => {
                warn("redis pubsub connect failed", &e);
                std::thread::sleep(backoff);
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}
