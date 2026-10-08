//! Local control feedback shares the app's reversible motion and theme tokens.
use super::*;
use crate::ui::motion::{self, Motion};
use gpui::{div, px, Bounds, Hsla, Pixels};
use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    ActiveTheme as _,
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

pub(super) const TIMES: [Option<u16>; 6] =
    [Some(15), Some(30), Some(45), Some(60), Some(120), None];

// Bounds are local to each group so scrolling moves the whole control together.
// Canvas measures targets before paint; only the selection quad moves, not labels.
struct SelectionMotion {
    selected: String,
    container: Bounds<Pixels>,
    choices: BTreeMap<String, Bounds<Pixels>>,
    from: Option<Bounds<Pixels>>,
    to: Option<Bounds<Pixels>>,
    progress: Motion,
}
impl SelectionMotion {
    fn new(selected: String) -> Self {
        Self {
            selected,
            container: Default::default(),
            choices: BTreeMap::new(),
            from: None,
            to: None,
            progress: Motion::new(1.),
        }
    }
    fn current(&self, reduced: bool) -> Option<Bounds<Pixels>> {
        let to = self.to?;
        let from = self.from.unwrap_or(to);
        let t = if reduced { 1. } else { self.progress.value() };
        Some(Bounds {
            origin: from.origin + (to.origin - from.origin) * t,
            size: gpui::size(
                from.size.width + (to.size.width - from.size.width) * t,
                from.size.height + (to.size.height - from.size.height) * t,
            ),
        })
    }
    fn select(&mut self, selected: String, animate: bool, cx: &mut gpui::Context<PlayNowView>) {
        if self.selected == selected {
            if !animate {
                self.progress.snap(1.);
            }
            return;
        }
        self.from = self.current(motion::reduced(cx));
        self.to = self.choices.get(&selected).copied();
        self.selected = selected;
        if animate && self.from.is_some() && self.to.is_some() {
            self.progress.snap(0.);
            self.progress.set(1., cx);
        } else {
            self.progress.snap(1.);
        }
    }
    fn paint_bounds(&mut self, reduced: bool) -> Option<Bounds<Pixels>> {
        let measured = self.choices.get(&self.selected).copied()?;
        // Initial mount and responsive reflow snap to measured geometry. A move
        // between choices uses the existing bounds, including wrapped rows.
        if self.to != Some(measured) {
            self.from = Some(measured);
            self.to = Some(measured);
            self.progress.snap(1.);
        }
        let mut bounds = self.current(reduced)?;
        bounds.origin += self.container.origin;
        Some(bounds)
    }
}

pub(super) struct ControlMotion {
    groups: BTreeMap<&'static str, Rc<RefCell<SelectionMotion>>>,
    brain: Motion,
}
impl ControlMotion {
    pub fn new(context: &rec::Context) -> Self {
        Self {
            groups: Self::targets(context)
                .into_iter()
                .map(|(group, selected)| {
                    (group, Rc::new(RefCell::new(SelectionMotion::new(selected))))
                })
                .collect(),
            brain: Motion::with_duration(f32::from(context.brain_dead), motion::CONTROL),
        }
    }
    fn targets(context: &rec::Context) -> [(&'static str, String); 5] {
        [
            ("time", format!("time-{:?}", context.minutes)),
            ("energy", format!("energy-{:?}", context.energy)),
            (
                "activity",
                context
                    .activity
                    .map_or_else(|| "activity-any".into(), |v| format!("activity-{v:?}")),
            ),
            ("device", format!("device-{:?}", context.device)),
            ("scope", format!("scope-{:?}", context.scope)),
        ]
    }
    pub fn retarget(
        &mut self,
        context: &rec::Context,
        animate: bool,
        cx: &mut gpui::Context<PlayNowView>,
    ) {
        let animate = animate && !motion::reduced(cx);
        for (group, selected) in Self::targets(context) {
            self.groups[group]
                .borrow_mut()
                .select(selected, animate, cx);
        }
        let target = f32::from(context.brain_dead);
        if !animate {
            self.brain.snap(target);
        } else if self.brain.target() != target {
            self.brain.set(target, cx);
        }
    }
}

impl PlayNowView {
    pub(super) fn selection_group(&self, group: &'static str, cx: &gpui::App) -> gpui::Div {
        let measure = self.controls.groups[group].clone();
        let paint = measure.clone();
        // Native macOS selects a segment with the accent fill (Apple UI kit).
        let color = if crate::theme::macos_shell(cx) {
            cx.theme().primary
        } else {
            cx.theme().secondary_active
        };
        gpui_component::h_flex().relative().child(
            gpui::canvas(
                move |bounds, _, _| {
                    measure.borrow_mut().container = bounds;
                },
                move |_, _, window, cx| {
                    if let Some(bounds) = paint.borrow_mut().paint_bounds(motion::reduced(cx)) {
                        window.paint_quad(gpui::fill(bounds, color).corner_radii(px(7.)));
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
    }
    pub(super) fn choice(
        &self,
        group: &'static str,
        id: String,
        segmented: bool,
        button: Button,
        cx: &gpui::App,
    ) -> gpui::Div {
        let measure = self.controls.groups[group].clone();
        let theme = cx.theme();
        let on_accent = crate::theme::macos_shell(cx) && measure.borrow().selected == id;
        // Keep the button surface clear so one pill remains visible in flight.
        // Hover and press are immediate, translucent overlays on that surface.
        let style = ButtonCustomVariant::new(cx)
            .color(theme.transparent)
            .foreground(if on_accent {
                theme.primary_foreground
            } else {
                theme.foreground
            })
            .border(if segmented {
                theme.transparent
            } else {
                theme.border
            })
            .hover(theme.secondary_hover.opacity(0.35))
            .active(theme.secondary_active.opacity(0.35));
        div()
            .relative()
            .flex_shrink_0()
            .when(segmented, |el| el.flex_1())
            .child(button.w_full().custom(style))
            .child(
                gpui::canvas(
                    move |mut bounds, _, _| {
                        let mut state = measure.borrow_mut();
                        bounds.origin -= state.container.origin;
                        state.choices.insert(id, bounds);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            )
    }
    pub(super) fn brain_switch(&self, cx: &mut gpui::Context<Self>) -> Button {
        let t = if motion::reduced(cx) {
            self.controls.brain.target()
        } else {
            self.controls.brain.value()
        };
        let theme = cx.theme();
        let native = crate::theme::macos_shell(cx);
        let off = if native {
            crate::ui::controls::switch_off(cx)
        } else {
            theme.secondary_active
        };
        // The pinned Switch has no keyboard or reduced-motion path. Reuse Button
        // input/focus with the same 36 x 20 track and 16 px thumb geometry.
        Button::new("brain-dead")
            .ghost()
            .w(px(44.))
            .h(px(40.))
            .p_0()
            .tooltip("Toggle Brain dead")
            .child(
                div()
                    .relative()
                    .w(px(36.))
                    .h(px(20.))
                    .rounded(px(10.))
                    .when(!native, |track| {
                        track
                            .border_1()
                            .border_color(theme.muted_foreground.opacity(0.4))
                    })
                    .bg(mix(off, theme.primary, t))
                    .child(
                        div()
                            .absolute()
                            .top(px(2.))
                            .left(px(2. + 16. * t))
                            .size(px(16.))
                            .rounded(px(8.))
                            .bg(if native {
                                gpui::white()
                            } else {
                                mix(theme.foreground, theme.primary_foreground, t)
                            })
                            .shadow_sm(),
                    ),
            )
            .on_click(cx.listener(|this, event, _, cx| {
                this.context.brain_dead = !this.context.brain_dead;
                this.change_context(Some(event), cx);
            }))
    }
}

fn mix(from: Hsla, to: Hsla, amount: f32) -> Hsla {
    from.blend(to.opacity(amount))
}
