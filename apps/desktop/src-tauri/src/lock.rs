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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Reason {
    /// User locked.
    Manual,
    /// Locked via the agent endpoint (terminal `ks lock`).
    Terminal,
    /// Idle timeout.
    Idle,
    /// Max session lifetime.
    Max,
    /// System sleep.
    #[cfg_attr(
        not(target_os = "macos"),
        expect(
            dead_code,
            reason = "emitted only by the macOS platform module until D7 adds Windows and Linux signals"
        )
    )]
    Sleep,
    /// Screen locked / session resigned.
    #[cfg_attr(
        not(target_os = "macos"),
        expect(
            dead_code,
            reason = "emitted only by the macOS platform module until D7 adds Windows and Linux signals"
        )
    )]
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

pub(crate) static APP: Mutex<Option<AppHandle>> = Mutex::new(None);
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
    if let Some(app) = &*APP
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
    {
        drop(app.run_on_main_thread(platform::end_activity));
    }
}

/// Locks via state; emits the event once. Returns false when already locked.
pub(crate) fn lock_now(state: &std::sync::Arc<AppState>, reason: Reason) -> bool {
    state.lock(reason)
}

pub(crate) struct Wake;

/// Idle elapsed time seen by the lock timers: the shorter of the app's
/// own activity clock and the agent's (terminal unwrap/wrap traffic
/// resets it through `touch`).
fn effective_elapsed(o: &crate::state::Open) -> Duration {
    let own = o.last_activity.elapsed();
    o.agent.as_ref().map_or(own, |a| own.min(a.idle()))
}

fn expired_reason(state: &std::sync::Arc<AppState>, idle: Duration) -> Reason {
    let i = state
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i.open.as_ref().map_or(Reason::Max, |o| {
        if effective_elapsed(o) >= idle {
            Reason::Idle
        } else {
            Reason::Max
        }
    })
}

fn time_left(state: &std::sync::Arc<AppState>, idle: Duration, max: Duration) -> Option<Duration> {
    let i = state
        .inner
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    i.open.as_ref().and_then(|o| {
        o.session.as_ref()?;
        let u = o.unlocked_at?;
        Some(
            idle.saturating_sub(effective_elapsed(o))
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
            let wait = match time_left(&state, idle, max) {
                None => Duration::from_secs(60),
                Some(d) if d.is_zero() => {
                    let _ = lock_now(&state, expired_reason(&state, idle));
                    return;
                }
                Some(d) => d.min(Duration::from_secs(5)),
            };
            if rx.recv_timeout(wait).is_ok() {
                return;
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
