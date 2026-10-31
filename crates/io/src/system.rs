//! OS integration for a live session.
//!
//! Runs on: control thread, once at startup.

/// Keeps the process fully awake while the returned guard lives: no App Nap throttling of the
/// control/UI threads, no idle system sleep. On other platforms this does nothing.
#[must_use = "the activity ends when the guard is dropped"]
pub fn keep_alive(reason: &str) -> impl Sized {
    platform::begin(reason)
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};

    pub fn begin(reason: &str) -> impl Sized {
        let options = NSActivityOptions::UserInitiated | NSActivityOptions::LatencyCritical;
        NSProcessInfo::processInfo()
            .beginActivityWithOptions_reason(options, &NSString::from_str(reason))
    }
}

#[cfg(not(target_os = "macos"))]
mod platform {
    pub fn begin(_reason: &str) -> impl Sized {}
}
