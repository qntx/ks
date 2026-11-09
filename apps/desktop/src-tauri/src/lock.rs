//! Lock timers and system signals. One timer thread owns idle/max
//! deadlines; platform hooks call `lock_now` for sleep/screen-lock.

use std::sync::Mutex;
use std::sync::mpsc::{Sender, channel};
use std::time::Duration;

use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Emitter as _};

use crate::state::AppState;

/// Why a lock happened (also the `Locked` event payload).
#[derive(Debug, Clone, Copy, Serialize, Type)]
#[serde(rename_all = "snake_case")]
#[expect(
    dead_code,
    reason = "ScreenLock/Quit fire from platform and lifecycle hooks in D2/D5"
)]
pub(crate) enum Reason {
    /// User locked.
    Manual,
    /// Idle timeout.
    Idle,
    /// Max session lifetime.
    Max,
    /// System sleep.
    Sleep,
    /// Screen locked / session resigned.
    ScreenLock,
    /// `.identity.age` changed under the session.
    KeyChanged,
    /// App quitting.
    Quit,
}

/// Emitted to the frontend whenever the vault locks.
#[derive(Debug, Clone, Serialize, Type, tauri_specta::Event)]
pub(crate) struct Locked {
    /// Why.
    pub reason: Reason,
}

static APP: Mutex<Option<AppHandle>> = Mutex::new(None);
/// Registers the AppHandle used for events.
pub(crate) fn init(app: &AppHandle) {
    *APP.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(app.clone());
}

/// Emits `Locked` and ends App Nap suppression.
pub(crate) fn on_locked(reason: Reason) {
    if let Some(app) = &*APP
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        drop(app.emit("locked", Locked { reason }));
    }
    #[cfg(target_os = "macos")]
    platform::end_activity();
}

/// Locks via state; emits the event once. Returns false when already locked.
pub(crate) fn lock_now(state: &AppState, reason: Reason) -> bool {
    state.lock(reason)
}

pub(crate) struct Wake;

fn time_left(state: &std::sync::Arc<AppState>, idle: Duration, max: Duration) -> Option<Duration> {
    let i = state
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i.open.as_ref().and_then(|o| {
        let u = o.unlocked_at?;
        Some(
            idle.saturating_sub(o.last_activity.elapsed())
                .min(max.saturating_sub(u.elapsed())),
        )
    })
}

/// Starts the idle/max watchdog for the current session. Returns a
/// sender that stops the thread.
pub(crate) fn start(
    state: std::sync::Arc<AppState>,
    idle: Duration,
    max: Duration,
) -> Sender<Wake> {
    let (tx, rx) = channel::<Wake>();
    std::thread::spawn(move || {
        loop {
            match time_left(&state, idle, max) {
                None => {
                    if rx.recv_timeout(Duration::from_secs(60)).is_ok() {
                        return;
                    }
                }
                Some(d) if d.is_zero() => {
                    let i = state
                        .inner
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    let idle_expired = i
                        .open
                        .as_ref()
                        .is_some_and(|o| o.last_activity.elapsed() >= idle);
                    drop(i);
                    lock_now(
                        &state,
                        if idle_expired {
                            Reason::Idle
                        } else {
                            Reason::Max
                        },
                    );
                }
                Some(d) => {
                    if rx.recv_timeout(d.min(Duration::from_secs(5))).is_ok() {
                        return;
                    }
                }
            }
        }
    });
    tx
}

/// macOS system signals (sleep/screen-lock/App Nap); no-op elsewhere.
#[cfg(target_os = "macos")]
#[path = "platform/macos.rs"]
#[allow(unsafe_code, reason = "objc2 message sends")]
pub(crate) mod platform;

/// Starts sleep/screen-lock listeners (macOS only; needs `lockOnSleep`
/// from settings passed in).
pub(crate) fn watch_system(state: std::sync::Arc<AppState>, lock_on_sleep: bool) {
    #[cfg(target_os = "macos")]
    platform::watch_system(state, lock_on_sleep);
    #[cfg(not(target_os = "macos"))]
    let _ = (state, lock_on_sleep);
}
