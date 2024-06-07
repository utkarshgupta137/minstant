// Copyright 2021 TiKV Project Authors. Licensed under Apache-2.0.

//! This module will be compiled when it's either linux_x86 or linux_x86_64.

use std::cell::UnsafeCell;
use std::time::Instant;

static TSC_STATE: TSCState = TSCState {
    cycles_per_second: UnsafeCell::new(1),
    cycles_from_anchor: UnsafeCell::new(1),
    nanos_per_cycle: UnsafeCell::new(1.0),
};

struct TSCState {
    cycles_per_second: UnsafeCell<u64>,
    cycles_from_anchor: UnsafeCell<u64>,
    nanos_per_cycle: UnsafeCell<f64>,
}

unsafe impl Sync for TSCState {}

#[ctor::ctor]
unsafe fn init() {
    let anchor = Instant::now();
    let (cps, cfa) = cycles_per_sec(anchor);
    *TSC_STATE.cycles_per_second.get() = cps;
    *TSC_STATE.cycles_from_anchor.get() = cfa;
    *TSC_STATE.nanos_per_cycle.get() = 1_000_000_000.0 / cps as f64;
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

#[inline]
pub(crate) fn nanos_per_cycle() -> f64 {
    unsafe { *TSC_STATE.nanos_per_cycle.get() }
}

#[inline]
pub(crate) fn cycles_from_anchor() -> u64 {
    unsafe { *TSC_STATE.cycles_from_anchor.get() }
}

#[inline]
pub(crate) fn current_cycle() -> u64 {
    tsc().wrapping_sub(cycles_from_anchor())
}

/// Returns (1) cycles per second and (2) cycles from anchor.
/// The result of subtracting `cycles_from_anchor` from newly fetched TSC
/// can be used to
///   1. readjust TSC to begin from zero
///   2. sync TSCs between all CPUs
fn cycles_per_sec(anchor: Instant) -> (u64, u64) {
    let (cps, last_monotonic, last_tsc) = _cycles_per_sec();
    let nanos_from_anchor = (last_monotonic - anchor).as_nanos();
    let cycles_flied = cps as f64 * nanos_from_anchor as f64 / 1_000_000_000.0;
    let cycles_from_anchor = last_tsc - cycles_flied.ceil() as u64;

    (cps, cycles_from_anchor)
}

/// Returns (1) cycles per second, (2) last monotonic time and (3) associated tsc.
fn _cycles_per_sec() -> (u64, Instant, u64) {
    let mut cycles_per_sec;
    let mut last_monotonic;
    let mut last_tsc;
    let mut old_cycles = 0.0;

    loop {
        let (t1, tsc1) = monotonic_with_tsc();
        loop {
            let (t2, tsc2) = monotonic_with_tsc();
            last_monotonic = t2;
            last_tsc = tsc2;
            let elapsed_nanos = (t2 - t1).as_nanos();
            if elapsed_nanos > 10_000_000 {
                cycles_per_sec = (tsc2 - tsc1) as f64 * 1_000_000_000.0 / elapsed_nanos as f64;
                break;
            }
        }
        let delta = f64::abs(cycles_per_sec - old_cycles);
        if delta / cycles_per_sec < 0.000001 {
            break;
        }
        old_cycles = cycles_per_sec;
    }

    (cycles_per_sec.round() as u64, last_monotonic, last_tsc)
}

/// Try to get tsc and monotonic time at the same time. Due to
/// get interrupted in half way may happen, they aren't guaranteed
/// to represent the same instant.
fn monotonic_with_tsc() -> (Instant, u64) {
    (Instant::now(), tsc())
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn tsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

#[cfg(target_arch = "aarch64")]
#[inline]
fn tsc() -> u64 {
    let count: u64;
    unsafe {
        ::core::arch::asm!("mrs {}, cntvct_el0", out(reg) count);
    }
    count
}
