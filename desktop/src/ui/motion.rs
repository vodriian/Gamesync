//! Reversible ease-out motion: 200 ms panels/selections, 120 ms switches, no subtree fade.
use gpui::{App, Context, Global, Window};
use std::time::{Duration, Instant};
pub const STANDARD: Duration = Duration::from_millis(200);
pub const CONTROL: Duration = Duration::from_millis(120);
#[derive(Default)]
pub struct MotionPreferences {
    pub reduced: bool,
}
impl Global for MotionPreferences {}
pub fn reduced(cx: &App) -> bool {
    cx.try_global::<MotionPreferences>()
        .is_some_and(|p| p.reduced)
}

/// Linux card projection is available only while the Omarchy design language is active.
pub fn card_3d_enabled(cx: &App) -> bool {
    !reduced(cx)
        && (cfg!(target_os = "macos")
            || (cfg!(target_os = "linux") && crate::theme::omarchy_mode(cx)))
}

/// A reversible value that eases to its target. It holds no timer: the view
/// that draws it calls `animate` in `render`, so frames follow the display
/// rate on every platform.
pub struct Motion {
    from: f32,
    target: f32,
    start: Instant,
    duration: Duration,
}
impl Motion {
    pub fn new(value: f32) -> Self {
        Self::with_duration(value, STANDARD)
    }
    pub fn with_duration(value: f32, duration: Duration) -> Self {
        Self {
            from: value,
            target: value,
            start: Instant::now(),
            duration,
        }
    }
    pub fn value(&self) -> f32 {
        self.value_after(self.start.elapsed())
    }
    fn value_after(&self, elapsed: Duration) -> f32 {
        let t = (elapsed.as_secs_f32() / self.duration.as_secs_f32()).min(1.);
        self.from + (self.target - self.from) * (1. - (1. - t).powi(5))
    }
    pub fn target(&self) -> f32 {
        self.target
    }
    /// True while the value still moves toward its target.
    pub fn active(&self) -> bool {
        self.active_after(self.start.elapsed())
    }
    fn active_after(&self, elapsed: Duration) -> bool {
        self.from != self.target && elapsed < self.duration
    }
    /// Request the next frame while the value still moves. Call in `render`
    /// of a view that draws this motion.
    pub fn animate(&self, window: &mut Window) {
        if self.active() {
            window.request_animation_frame();
        }
    }
    pub fn snap(&mut self, value: f32) {
        self.from = value;
        self.target = value;
    }
    pub fn set<T: 'static>(&mut self, value: f32, cx: &mut Context<T>) {
        // Reduced motion starts at the target, so the motion is never active.
        self.from = if reduced(cx) { value } else { self.value() };
        self.target = value;
        self.start = Instant::now();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moving(from: f32, target: f32) -> Motion {
        Motion {
            from,
            target,
            start: Instant::now(),
            duration: STANDARD,
        }
    }

    #[test]
    fn a_motion_is_active_until_its_duration_ends_then_rests_on_target() {
        let motion = moving(0., 1.);
        assert_eq!(motion.value_after(Duration::ZERO), 0.);
        assert!(motion.active_after(Duration::ZERO));
        let mut previous = 0.;
        for ms in (10..200).step_by(10) {
            let elapsed = Duration::from_millis(ms);
            let value = motion.value_after(elapsed);
            assert!(value > previous && value < 1., "{ms} ms: {value}");
            assert!(motion.active_after(elapsed));
            previous = value;
        }
        for elapsed in [STANDARD, STANDARD * 3] {
            assert_eq!(motion.value_after(elapsed), 1.);
            assert!(!motion.active_after(elapsed));
        }
    }

    #[test]
    fn a_resting_or_snapped_motion_requests_no_frames() {
        assert!(!Motion::new(1.).active());
        let mut motion = moving(0., 1.);
        assert!(motion.active());
        motion.snap(0.4);
        assert!(!motion.active());
        assert_eq!((motion.value(), motion.target()), (0.4, 0.4));
    }

    #[test]
    fn a_reversal_starts_from_the_current_value() {
        // `set` reads `value()` as the new start; the eased value is continuous.
        let motion = moving(0., 1.);
        let halfway = motion.value_after(STANDARD / 2);
        let back = moving(halfway, 0.);
        assert_eq!(back.value_after(Duration::ZERO), halfway);
        assert_eq!(back.value_after(STANDARD), 0.);
    }
}
