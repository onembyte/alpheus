//! Runs storage-detail work below the app's own priority.
//!
//! macOS schedules threads by quality of service. Utility is Apple's class for
//! long work the user sees progress of: it yields CPU and disk to the window
//! and to the reclaim scan. Elsewhere these are no-ops.

/// Lowers the calling thread to utility QoS.
pub fn lower() {
    #[cfg(target_os = "macos")]
    // SAFETY: only changes the calling thread's own scheduling class.
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_UTILITY, 0);
    }
}

/// Whether the calling thread runs at utility QoS or below.
pub fn is_lowered() -> bool {
    #[cfg(target_os = "macos")]
    {
        use libc::qos_class_t::{QOS_CLASS_BACKGROUND, QOS_CLASS_UNSPECIFIED, QOS_CLASS_UTILITY};
        let mut class = QOS_CLASS_UNSPECIFIED;
        let mut priority = 0;
        // SAFETY: reads the calling thread's own class into locals.
        let rc = unsafe {
            libc::pthread_get_qos_class_np(libc::pthread_self(), &mut class, &mut priority)
        };
        rc == 0 && matches!(class, QOS_CLASS_UTILITY | QOS_CLASS_BACKGROUND)
    }
    #[cfg(not(target_os = "macos"))]
    false
}
