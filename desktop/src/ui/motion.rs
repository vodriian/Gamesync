//! Reversible ease-out motion: 200 ms panels/selections, 120 ms switches, no subtree fade.
use gpui::{App, Context, Global, Task};
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

pub struct Motion {
    from: f32,
    target: f32,
    start: Instant,
    duration: Duration,
    task: Option<Task<()>>,
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
            task: None,
        }
    }
    pub fn value(&self) -> f32 {
        let t = (self.start.elapsed().as_secs_f32() / self.duration.as_secs_f32()).min(1.);
        self.from + (self.target - self.from) * (1. - (1. - t).powi(5))
    }
    pub fn target(&self) -> f32 {
        self.target
    }
    pub fn snap(&mut self, value: f32) {
        self.from = value;
        self.target = value;
        self.task = None;
    }
    pub fn set<T: 'static>(&mut self, value: f32, cx: &mut Context<T>) {
        self.from = if reduced(cx) { value } else { self.value() };
        self.target = value;
        self.start = Instant::now();
        self.task = None;
        if reduced(cx) {
            cx.notify();
            return;
        }
        let duration = self.duration;
        self.task = Some(cx.spawn(async move |this, cx| {
            let start = Instant::now();
            while start.elapsed() < duration {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        }));
        cx.notify();
    }
}
