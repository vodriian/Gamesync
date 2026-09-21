//! Eagle's 200 ms ease-out panel motion, with continuous reversal and no subtree fade.
use gpui::{App, Context, Global, Task};
use std::time::{Duration, Instant};
pub const STANDARD: Duration = Duration::from_millis(200);
#[derive(Default)]
pub struct MotionPreferences {
    pub reduced: bool,
}
impl Global for MotionPreferences {}
pub fn reduced(cx: &App) -> bool {
    cx.try_global::<MotionPreferences>()
        .is_some_and(|p| p.reduced)
}

pub struct Motion {
    from: f32,
    target: f32,
    start: Instant,
    task: Option<Task<()>>,
}
impl Motion {
    pub fn new(value: f32) -> Self {
        Self {
            from: value,
            target: value,
            start: Instant::now(),
            task: None,
        }
    }
    pub fn value(&self) -> f32 {
        let t = (self.start.elapsed().as_secs_f32() / STANDARD.as_secs_f32()).min(1.);
        self.from + (self.target - self.from) * (1. - (1. - t).powi(5))
    }
    pub fn active(&self) -> bool {
        self.from != self.target && self.start.elapsed() < STANDARD
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
        self.task = Some(cx.spawn(async move |this, cx| {
            let start = Instant::now();
            while start.elapsed() < STANDARD {
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
