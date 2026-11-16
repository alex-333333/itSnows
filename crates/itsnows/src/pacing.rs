//! Decisions about when to draw and when to stay hidden, kept free of OS
//! calls so they can be tested anywhere.

/// Values of `QUERY_USER_NOTIFICATION_STATE`.
pub const QUNS_NOT_PRESENT: i32 = 1;
pub const QUNS_BUSY: i32 = 2;
pub const QUNS_RUNNING_D3D_FULL_SCREEN: i32 = 3;
pub const QUNS_PRESENTATION_MODE: i32 = 4;

/// Shortest time one frame may take: just under 1/60 s on mains and 1/30 s
/// on battery, so a 60 Hz display's own vsync still sets the rhythm while a
/// 144 or 240 Hz display cannot drive the app faster than intended.
const FRAME_MS_MAINS: f32 = 15.0;
const FRAME_MS_BATTERY: f32 = 31.0;

/// How long to wait after a frame that took `elapsed_s` before starting the next.
pub fn frame_wait_ms(elapsed_s: f32, on_battery: bool) -> u32 {
    let target = if on_battery { FRAME_MS_BATTERY } else { FRAME_MS_MAINS };
    if elapsed_s.is_nan() {
        return 0;
    }
    (target - elapsed_s * 1000.0).clamp(0.0, target).round() as u32
}

/// The overlay should be hidden: something is full-screen, a presentation is
/// running, or nobody is there to see it (locked, screensaver, disconnected).
pub fn hides_overlay(notification_state: i32) -> bool {
    matches!(notification_state, QUNS_NOT_PRESENT | QUNS_BUSY | QUNS_RUNNING_D3D_FULL_SCREEN | QUNS_PRESENTATION_MODE)
}

/// While nothing is moving, a frame is only worth presenting if it differs
/// from the one already on screen.
pub fn should_present<T: PartialEq>(idle: bool, frame: &[T], shown: &[T]) -> bool {
    !idle || frame != shown
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUNS_ACCEPTS_NOTIFICATIONS: i32 = 5;
    const QUNS_QUIET_TIME: i32 = 6;
    const QUNS_APP: i32 = 7;

    #[test]
    fn mains_waits_out_the_rest_of_a_60fps_frame() {
        assert_eq!(frame_wait_ms(0.004, false), 11);
        assert_eq!(frame_wait_ms(0.0, false), 15);
    }

    #[test]
    fn battery_waits_out_the_rest_of_a_30fps_frame() {
        assert_eq!(frame_wait_ms(0.004, true), 27);
    }

    #[test]
    fn no_wait_when_the_frame_already_took_long_enough() {
        assert_eq!(frame_wait_ms(0.0166, false), 0);
        assert_eq!(frame_wait_ms(0.5, false), 0);
        assert_eq!(frame_wait_ms(0.040, true), 0);
    }

    #[test]
    fn a_fast_display_cannot_push_past_the_cap() {
        // 240 Hz: a frame comes back after ~4 ms. With the wait added, at most ~66 frames fit in a second.
        let per_frame = 0.004 + frame_wait_ms(0.004, false) as f32 / 1000.0;
        assert!(1.0 / per_frame <= 67.0);
    }

    #[test]
    fn odd_elapsed_values_are_safe() {
        assert_eq!(frame_wait_ms(f32::NAN, false), 0);
        assert_eq!(frame_wait_ms(-1.0, false), 15);
        assert_eq!(frame_wait_ms(f32::INFINITY, true), 0);
    }

    #[test]
    fn hides_for_fullscreen_presentation_and_absence() {
        assert!(hides_overlay(QUNS_NOT_PRESENT));
        assert!(hides_overlay(QUNS_BUSY));
        assert!(hides_overlay(QUNS_RUNNING_D3D_FULL_SCREEN));
        assert!(hides_overlay(QUNS_PRESENTATION_MODE));
    }

    #[test]
    fn stays_for_normal_desktop_states() {
        assert!(!hides_overlay(QUNS_ACCEPTS_NOTIFICATIONS));
        assert!(!hides_overlay(QUNS_QUIET_TIME));
        assert!(!hides_overlay(QUNS_APP));
        assert!(!hides_overlay(0));
        assert!(!hides_overlay(99));
    }

    #[test]
    fn presents_only_what_changed_while_idle() {
        let a = vec![1, 2, 3];
        assert!(should_present(false, &a, &a), "moving scenes always present");
        assert!(!should_present(true, &a, &a), "idle and identical: skip");
        assert!(should_present(true, &a, &[1, 2]), "idle but different: present once");
        assert!(should_present(true, &[], &a));
    }
}
