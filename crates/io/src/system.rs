//! OS integration for a live session.
//!
//! Runs on: control thread.

/// Keeps the process fully awake while the returned guard lives: no App Nap throttling of the
/// control/UI threads, no idle system sleep. On other platforms this does nothing.
#[must_use = "the activity ends when the guard is dropped"]
pub fn keep_alive(reason: &str) -> impl Sized {
    platform::begin(reason)
}

/// Current host time in µs, on the clock of CoreMIDI and CoreAudio timestamps (0 elsewhere).
#[must_use]
pub fn host_time_us() -> u64 {
    platform::host_time_us()
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};

    pub fn begin(reason: &str) -> impl Sized {
        let options = NSActivityOptions::UserInitiated | NSActivityOptions::LatencyCritical;
        NSProcessInfo::processInfo()
            .beginActivityWithOptions_reason(options, &NSString::from_str(reason))
    }

    #[repr(C)]
    #[derive(Default)]
    struct MachTimebaseInfo {
        numer: u32,
        denom: u32,
    }

    unsafe extern "C" {
        fn mach_absolute_time() -> u64;
        fn mach_timebase_info(info: *mut MachTimebaseInfo) -> i32;
    }

    /// Same conversion as cpal (`mach_timebase_info`) and CoreMIDI (`AudioConvertHostTimeToNanos`).
    pub fn host_time_us() -> u64 {
        let mut info = MachTimebaseInfo::default();
        // SAFETY: both are plain libSystem calls; `info` is a valid, writable struct.
        let (ticks, status) = unsafe { (mach_absolute_time(), mach_timebase_info(&mut info)) };
        if status != 0 || info.denom == 0 {
            return 0;
        }
        let nanos = u128::from(ticks) * u128::from(info.numer) / u128::from(info.denom);
        u64::try_from(nanos / 1_000).unwrap_or(u64::MAX)
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn begin(_reason: &str) -> impl Sized {}

    pub fn host_time_us() -> u64 {
        0
    }
}
