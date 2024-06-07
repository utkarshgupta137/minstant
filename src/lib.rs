// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

//! A drop-in replacement for [`std::time::Instant`](https://doc.rust-lang.org/std/time/struct.Instant.html)
//! that measures time with high performance and high accuracy powered by [TSC](https://en.wikipedia.org/wiki/Time_Stamp_Counter).
//!
//! ## Example
//!
//! ```
//! let start = minstant::Instant::now();
//!
//! // Code snipppet to measure
//!
//! let duration: std::time::Duration = start.elapsed();
//! ```
//!
//! ## Platform Support
//!
//! Currently, only the Linux on `x86` or `x86_64` is backed by [TSC](https://en.wikipedia.org/wiki/Time_Stamp_Counter).
//! On other platforms, `minstant` falls back to coarse time.
//!
//! ## Calibration
//!
//! [TSC](https://en.wikipedia.org/wiki/Time_Stamp_Counter) doesn’t necessarily ticks in constant speed and even
//! doesn't synchronize across CPU cores. The calibration detects the TSC deviation and calculates the correction
//! factors with the assistance of a source wall clock. Once the deviation is beyond a crazy threshold, the calibration
//! will fail, and then we will fall back to coarse time.
//!
//! This calibration is stored globally and reused. In order to start the calibration before any call to `minstant`
//! as to make sure that the time spent on `minstant` is constant, we link the calibration into application's
//! initialization linker section, so it'll get executed once the process starts.
//!
//! *[See also the `Instant` type](crate::Instant).*

mod instant;
mod tsc_now;

pub use instant::{Anchor, Instant};

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use std::time::{Duration, Instant as StdInstant};

    #[test]
    fn test_monotonic() {
        let mut prev = 0;
        for _ in 0..10000 {
            let cur = crate::tsc_now::current_cycle();
            assert!(cur >= prev);
            prev = cur;
        }
    }

    #[test]
    fn test_nanos_per_cycle() {
        let _ = crate::tsc_now::nanos_per_cycle();
    }

    #[test]
    fn test_unix_time() {
        let now = Instant::now();
        let anchor = Anchor::new();
        let unix_nanos = now.as_unix_nanos(&anchor);
        assert!(unix_nanos > 0);
    }

    #[test]
    fn test_duration() {
        let mut rng = rand::thread_rng();
        for _ in 0..10 {
            let instant = Instant::now();
            let std_instant = StdInstant::now();
            std::thread::sleep(Duration::from_millis(rng.gen_range(100..500)));
            let check = move || {
                let duration_ns_minstant = instant.elapsed();
                let duration_ns_std = std_instant.elapsed();

                #[cfg(target_os = "windows")]
                let expect_max_delta_ns = 40_000_000;
                #[cfg(not(target_os = "windows"))]
                let expect_max_delta_ns = 5_000_000;

                let real_delta = (duration_ns_std.as_nanos() as i128
                    - duration_ns_minstant.as_nanos() as i128)
                    .abs();
                assert!(
                    real_delta < expect_max_delta_ns,
                    "real delta: {}",
                    real_delta
                );
            };
            check();
            std::thread::spawn(check).join().expect("join failed");
        }
    }
}
