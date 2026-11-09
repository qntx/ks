//! macOS: direct `NSPasteboard` writes with privacy markers.
//!
//! `unsafe` is permitted in this module alone (crate-level deny); the
//! generated objc2-app-kit 0.3 bindings make these calls safe, so none
//! is used today.
#![allow(unsafe_code, reason = "objc2 bindings are permitted here")]

use objc2_app_kit::{NSPasteboard, NSPasteboardContentsOptions, NSPasteboardTypeString};
use objc2_foundation::{NSData, NSString};

use crate::Error;

/// The pasteboard `changeCount` captured right after our write.
pub(crate) type Token = isize;

pub(crate) fn copy(text: &str) -> Result<Token, Error> {
    let pb = NSPasteboard::generalPasteboard();
    let opts = NSPasteboardContentsOptions(NSPasteboardContentsOptions::CurrentHostOnly.0);
    let _ = pb.clearContents();
    let _ = pb.prepareForNewContentsWithOptions(opts);
    let value = NSString::from_str(text);
    // SAFETY: `NSPasteboardTypeString` is a documented extern static of
    // immutable NSString; dereferencing it is sound.
    let string_ty = unsafe { NSPasteboardTypeString };
    if !pb.setString_forType(&value, string_ty) {
        return Err(Error::Unavailable("NSPasteboard rejected the write".into()));
    }
    for marker in [
        "org.nspasteboard.ConcealedType",
        "org.nspasteboard.TransientType",
    ] {
        let ty = NSString::from_str(marker);
        let _ = pb.setData_forType(Some(&NSData::data()), &ty);
    }
    Ok(pb.changeCount())
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "the non-macOS counterpart can fail; one signature across platforms"
)]
pub(crate) fn clear_if_unchanged(token: Token) -> Result<bool, Error> {
    let pb = NSPasteboard::generalPasteboard();
    if pb.changeCount() != token {
        return Ok(false);
    }
    let _ = pb.clearContents();
    Ok(true)
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::missing_assert_message,
    unsafe_code,
    reason = "test harness: panicking asserts and a direct NSPasteboard check"
)]
mod tests {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
    use objc2_foundation::NSString;

    fn save() -> Option<String> {
        let pb = NSPasteboard::generalPasteboard();
        // SAFETY: `NSPasteboardTypeString` is an immutable extern static.
        unsafe { pb.stringForType(NSPasteboardTypeString) }.map(|s| s.to_string())
    }

    fn restore(prev: Option<String>) {
        let pb = NSPasteboard::generalPasteboard();
        let _ = pb.clearContents();
        if let Some(s) = prev {
            // SAFETY: `NSPasteboardTypeString` is an immutable extern static.
            let _ =
                pb.setString_forType(&NSString::from_str(&s), unsafe { NSPasteboardTypeString });
        }
    }

    fn has_type(marker: &str) -> bool {
        let pb = NSPasteboard::generalPasteboard();
        pb.dataForType(&NSString::from_str(marker)).is_some()
    }

    fn current_string() -> Option<String> {
        let pb = NSPasteboard::generalPasteboard();
        // SAFETY: `NSPasteboardTypeString` is an immutable extern static.
        unsafe { pb.stringForType(NSPasteboardTypeString) }.map(|s| s.to_string())
    }

    // One test on purpose: both halves mutate the real system
    // pasteboard and must not race each other.
    #[test]
    fn copy_marks_clears_and_respects_foreign_writes() {
        let prev = save();
        let copied = crate::copy("s3cret-value").expect("copy failed");
        assert_eq!(current_string().as_deref(), Some("s3cret-value"));
        assert!(
            has_type("org.nspasteboard.ConcealedType"),
            "concealed missing"
        );
        assert!(
            has_type("org.nspasteboard.TransientType"),
            "transient missing"
        );
        assert!(
            crate::clear_if_unchanged(&copied).expect("clear"),
            "should have cleared"
        );
        assert_eq!(current_string(), None);

        let copied2 = crate::copy("first").expect("copy");
        let _other = crate::copy("second").expect("copy");
        assert!(
            !crate::clear_if_unchanged(&copied2).expect("check"),
            "foreign write must not be cleared"
        );
        assert_eq!(current_string().as_deref(), Some("second"));
        restore(prev);
    }
}
