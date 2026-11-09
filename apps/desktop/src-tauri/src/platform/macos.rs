//! macOS-only lock signals and App Nap suppression (objc2). `unsafe`
//! is allowed in this module alone inside ks-desktop.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSProcessInfo, NSString};

/// `beginActivity` returns a retained token usable from any thread;
/// AppKit documents activity objects as thread-safe.
struct SendObj(Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>);
// SAFETY: only used to carry the activity token between unlock and lock;
// `endActivity` is thread-safe per NSProcessInfo docs.
unsafe impl Send for SendObj {}
// SAFETY: shared access is serialized through the Mutex.
unsafe impl Sync for SendObj {}

use crate::lock::{Reason, lock_now};
use crate::state::AppState;

static ACTIVITY: std::sync::Mutex<SendObj> = std::sync::Mutex::new(SendObj(None));
static WATCHING: AtomicBool = AtomicBool::new(false);

/// NSWorkspace block observers for sleep, screen sleep and session
/// resign — all mean "the user walked away".
pub(crate) fn watch_system(state: Arc<AppState>, lock_on_sleep: bool) {
    if WATCHING.swap(true, Ordering::SeqCst) || !lock_on_sleep {
        return;
    }
    let _ = MainThreadMarker::new();
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    for name in [
        "NSWorkspaceWillSleepNotification",
        "NSWorkspaceScreensDidSleepNotification",
        "NSWorkspaceSessionDidResignActiveNotification",
    ] {
        let st = Arc::clone(&state);
        let block = RcBlock::new(move |_| {
            lock_now(&st, Reason::Sleep);
        });
        // SAFETY: `addObserverForName:object:queue:usingBlock:` is an unsafe
        // objc2 send because block lifetime/retention is manual; `block` is
        // retained by the call and the observer is leaked intentionally.
        unsafe {
            let obs = center.addObserverForName_object_queue_usingBlock(
                Some(&NSString::from_str(name)),
                None,
                None,
                &block,
            );
            // Keep the observer alive for the process lifetime.
            #[expect(
                clippy::disallowed_methods,
                reason = "NSWorkspace observers have no deregistration hook before process end; leaking is the intended lifetime"
            )]
            std::mem::forget(obs);
        }
    }
}

/// Suppresses App Nap while unlocked so timers/socket stay live.
pub(crate) fn begin_activity() {
    let info = NSProcessInfo::processInfo();
    let opts = objc2_foundation::NSActivityOptions(
        objc2_foundation::NSActivityOptions::UserInitiated.0
            | objc2_foundation::NSActivityOptions::IdleSystemSleepDisabled.0,
    );
    let act =
        info.beginActivityWithOptions_reason(opts, &NSString::from_str("ks session unlocked"));
    *ACTIVITY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = SendObj(Some(act));
}

/// Ends App Nap suppression on lock.
pub(crate) fn end_activity() {
    let Some(act) = ACTIVITY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .0
        .take()
    else {
        return;
    };
    unsafe {
        NSProcessInfo::processInfo().endActivity(&act);
    }
}
