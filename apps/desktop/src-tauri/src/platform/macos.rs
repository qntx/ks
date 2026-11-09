//! macOS-only lock signals, App Nap suppression and tray lifecycle
//! (objc2). Every AppKit object in this module is created and dropped
//! on the main thread, so no `Send`/`Sync` impls are needed anywhere.
//! `unsafe` stays scoped to single message sends with SAFETY notes.

use std::cell::RefCell;
use std::sync::Arc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSProcessInfo, NSString};

use crate::lock::{Reason, lock_now};
use crate::state::AppState;

thread_local! {
    /// Observer tokens kept for the process lifetime; created and
    /// dropped on the main thread (RunEvent::Exit / drop paths run
    /// there), so they never cross threads.
    static OBSERVERS: RefCell<Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>> = const { RefCell::new(Vec::new()) };
    /// The NSProcessInfo activity suppressing App Nap while unlocked.
    static ACTIVITY: RefCell<Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>> =
        const { RefCell::new(None) };
}

fn observe(
    _mtm: MainThreadMarker,
    names: &[&str],
    dist: bool,
    state: &Arc<AppState>,
    reason: Reason,
) {
    for name in names {
        let st = Arc::clone(state);
        let block = RcBlock::new(move |_| {
            lock_now(&st, reason);
        });
        // SAFETY: unsafe because the block/object lifetimes are manual;
        // `block` is retained by the call and the returned token is kept
        // in `OBSERVERS` on this (main) thread for the process lifetime.
        let obs = if dist {
            unsafe {
                objc2_foundation::NSDistributedNotificationCenter::defaultCenter()
                    .addObserverForName_object_queue_usingBlock(
                        Some(&NSString::from_str(name)),
                        None,
                        None,
                        &block,
                    )
            }
        } else {
            unsafe {
                NSWorkspace::sharedWorkspace()
                    .notificationCenter()
                    .addObserverForName_object_queue_usingBlock(
                        Some(&NSString::from_str(name)),
                        None,
                        None,
                        &block,
                    )
            }
        };
        OBSERVERS.with(|o| o.borrow_mut().push(obs));
    }
}

/// Registers all walk-away signals. Must run on the main thread
/// (called from `setup`).
pub(crate) fn watch_system(state: Arc<AppState>, lock_on_sleep: bool) {
    if !lock_on_sleep {
        return;
    }
    let Some(_mtm) = MainThreadMarker::new() else {
        return;
    };
    observe(
        _mtm,
        &[
            "NSWorkspaceWillSleepNotification",
            "NSWorkspaceScreensDidSleepNotification",
        ],
        false,
        &state,
        Reason::Sleep,
    );
    observe(
        _mtm,
        &["NSWorkspaceSessionDidResignActiveNotification"],
        false,
        &state,
        Reason::ScreenLock,
    );
    observe(
        _mtm,
        &["com.apple.screenIsLocked"],
        true,
        &state,
        Reason::ScreenLock,
    );
}

/// Suppresses App Nap while unlocked (must run on the main thread).
pub(crate) fn begin_activity() {
    debug_assert!(MainThreadMarker::new().is_some());
    let act = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
        objc2_foundation::NSActivityOptions::UserInitiated
            | objc2_foundation::NSActivityOptions::IdleSystemSleepDisabled,
        &NSString::from_str("ks session unlocked"),
    );
    ACTIVITY.with(|a| *a.borrow_mut() = Some(act));
}

/// Ends App Nap suppression (must run on the main thread).
pub(crate) fn end_activity() {
    ACTIVITY.with(|a| {
        if let Some(act) = a.borrow_mut().take() {
            // SAFETY: `endActivity` is documented thread-safe and the
            // activity token was created on this thread.
            unsafe {
                NSProcessInfo::processInfo().endActivity(&act);
            }
        }
    });
}
