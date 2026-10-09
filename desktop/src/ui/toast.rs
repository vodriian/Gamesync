//! Window-level feedback. Toasts do not take focus; hover and an inactive window
//! pause dismissal so completion messages remain available to read.
use super::motion::{self, Motion};
use gpui::{
    div, point, prelude::*, px, BoxShadow, Entity, Global, SharedString, Subscription, Task, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex, v_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _,
    StyledExt as _,
};
use std::time::{Duration, Instant};

const EDGE: f32 = 24.;
const WIDTH: f32 = 356.;
const HEIGHT: f32 = 88.;
const GAP: f32 = 8.;
const VISIBLE: Duration = Duration::from_secs(5);
const MAX_VISIBLE: usize = 3;

pub struct ToastGlobal(pub Entity<ToastHost>);
impl Global for ToastGlobal {}

#[derive(Clone, Copy)]
pub enum Kind {
    Success,
    Info,
    Warning,
    Error,
}

struct Toast {
    id: u64,
    kind: Kind,
    title: SharedString,
    description: Option<SharedString>,
    motion: Motion,
    offset: Motion,
    closing: bool,
    expires: Instant,
    paused_at: Option<Instant>,
}

pub struct ToastHost {
    toasts: Vec<Toast>,
    next_id: u64,
    hovered: Option<u64>,
    active: bool,
    timer: Option<Task<()>>,
    _activation: Subscription,
}

impl ToastHost {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            this.active = window.is_window_active();
            this.pause_timers();
            cx.notify();
        });
        Self {
            toasts: Vec::new(),
            next_id: 0,
            hovered: None,
            active: window.is_window_active(),
            timer: None,
            _activation: activation,
        }
    }

    pub fn show(
        &mut self,
        kind: Kind,
        title: impl Into<SharedString>,
        description: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let was_empty = self.toasts.is_empty();
        if self.toasts.len() == MAX_VISIBLE {
            self.toasts.remove(0);
        }
        let now = Instant::now();
        let mut motion = Motion::new(0.);
        motion.set(1., cx);
        self.next_id += 1;
        self.toasts.push(Toast {
            id: self.next_id,
            kind,
            title: title.into(),
            description,
            motion,
            offset: Motion::new(0.),
            closing: false,
            expires: now + VISIBLE,
            paused_at: (!self.active || self.hovered.is_some()).then_some(now),
        });
        self.reposition(cx);
        if was_empty {
            self.timer = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let keep_running = this
                    .update(cx, |this, cx| {
                        this.tick(cx);
                        !this.toasts.is_empty()
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }));
        }
        cx.notify();
    }

    fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        if motion::reduced(cx) {
            self.toasts.retain(|toast| toast.id != id);
            self.reposition(cx);
        } else if let Some(toast) = self
            .toasts
            .iter_mut()
            .find(|toast| toast.id == id && !toast.closing)
        {
            toast.closing = true;
            // Retarget from the visible position if dismissed during entry.
            toast.motion.set(0., cx);
        }
        cx.notify();
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let expired: Vec<_> = self
            .toasts
            .iter()
            .filter(|toast| !toast.closing && toast.paused_at.is_none() && now >= toast.expires)
            .map(|toast| toast.id)
            .collect();
        for id in expired {
            self.dismiss(id, cx);
        }
        let count = self.toasts.len();
        self.toasts
            .retain(|toast| !(toast.closing && toast.motion.value() <= 0.001));
        if count != self.toasts.len() {
            self.reposition(cx);
            cx.notify();
        }
    }

    fn reposition(&mut self, cx: &mut Context<Self>) {
        let count = self.toasts.len();
        for (index, toast) in self.toasts.iter_mut().enumerate() {
            let offset = (count - index - 1) as f32 * (HEIGHT + GAP);
            if toast.offset.target() != offset {
                toast.offset.set(offset, cx);
            }
        }
        if self
            .hovered
            .is_some_and(|id| !self.toasts.iter().any(|toast| toast.id == id))
        {
            self.hovered = None;
            self.pause_timers();
        }
    }

    fn pause_timers(&mut self) {
        let now = Instant::now();
        let paused = self.hovered.is_some() || !self.active;
        for toast in &mut self.toasts {
            if paused {
                toast.paused_at.get_or_insert(now);
            } else if let Some(start) = toast.paused_at.take() {
                toast.expires += now - start;
            }
        }
    }

    fn card(&self, toast: &Toast, cx: &mut Context<Self>) -> impl IntoElement {
        let progress = if motion::reduced(cx) {
            toast.motion.target()
        } else {
            toast.motion.value()
        };
        let (icon, color) = match toast.kind {
            Kind::Success => (IconName::CircleCheck, cx.theme().success),
            Kind::Info => (IconName::Info, cx.theme().info),
            Kind::Warning => (IconName::TriangleAlert, cx.theme().warning),
            Kind::Error => (IconName::CircleX, cx.theme().danger),
        };
        let id = toast.id;
        let offset = if motion::reduced(cx) {
            toast.offset.target()
        } else {
            toast.offset.value()
        };
        // Fade individual paints, not subtree opacity: GPUI does not composite
        // overlapping children into one translucent layer.
        h_flex()
            .id(("toast", id))
            .absolute()
            .occlude()
            .bottom(px(offset - 32. * (1. - progress)))
            .w_full()
            .h(px(HEIGHT))
            .p_4()
            .gap_3()
            .items_center()
            .on_hover(cx.listener(move |this, hovered, _, cx| {
                if *hovered {
                    this.hovered = Some(id);
                } else if this.hovered == Some(id) {
                    this.hovered = None;
                }
                this.pause_timers();
                cx.notify();
            }))
            .rounded(crate::theme::interface_radius(cx, px(10.)))
            .border_1()
            .border_color(cx.theme().border.opacity(progress))
            .bg(cx.theme().popover.opacity(progress))
            .text_color(cx.theme().popover_foreground.opacity(progress))
            .shadow(vec![BoxShadow {
                color: gpui::black().opacity(0.12 * progress),
                offset: point(px(0.), px(4.)),
                blur_radius: px(12.),
                spread_radius: px(0.),
            }])
            .child(
                Icon::new(icon)
                    .size(px(18.))
                    .text_color(color.opacity(progress)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .font_medium()
                            .truncate()
                            .child(toast.title.clone()),
                    )
                    .when_some(toast.description.clone(), |column, description| {
                        column.child(
                            div()
                                .text_xs()
                                .line_height(px(16.))
                                .max_h(px(32.))
                                .overflow_hidden()
                                .text_color(cx.theme().muted_foreground.opacity(progress))
                                .child(description),
                        )
                    }),
            )
            .child(
                Button::new(("dismiss-toast", id))
                    .ghost()
                    .xsmall()
                    .icon(IconName::Close)
                    .text_color(cx.theme().muted_foreground.opacity(progress))
                    .tooltip("Dismiss notification")
                    .disabled(toast.closing)
                    .on_click(cx.listener(move |this, _, _, cx| this.dismiss(id, cx))),
            )
    }
}

impl Render for ToastHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("toasts")
            .absolute()
            .bottom(px(EDGE))
            .right(px(EDGE))
            .w(px(WIDTH.min(
                (f32::from(window.viewport_size().width) - EDGE * 2.).max(0.),
            )))
            .h(px(if self.toasts.is_empty() {
                0.
            } else {
                HEIGHT + (self.toasts.len() - 1) as f32 * (HEIGHT + GAP)
            }))
            .children(self.toasts.iter().map(|toast| self.card(toast, cx)))
    }
}
