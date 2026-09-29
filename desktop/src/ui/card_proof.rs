//! Isolated native renderer acceptance fixture: one real game, including title and footer.
use super::card_motion::Spring;
use gpui::{prelude::*, *};
use gpui_component::{v_flex, Root};
struct Proof {
    game: crate::model::Game,
    turn: Spring,
    pitch: Spring,
    yaw: Spring,
    hover_bounds: Bounds<Pixels>,
    back: bool,
    focus: FocusHandle,
}
impl Render for Proof {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.turn.active() || self.pitch.active() || self.yaw.active() {
            window.request_animation_frame();
        }
        let turn = self.turn.value();
        let back = turn > std::f32::consts::FRAC_PI_2;
        let face = if back {
            v_flex()
                .w(px(380.))
                .h(px(554.8))
                .p_6()
                .gap_4()
                .bg(rgb(0xe6e4df))
                .text_color(rgb(0x20242c))
                .child(
                    div()
                        .font_family("Georgia")
                        .text_3xl()
                        .child(self.game.title.clone()),
                )
                .child(self.game.description.clone())
                .child("★ 4.5 / 5 · Playing")
                .child("Native card rear — readable after a full 180° turn")
                .into_any_element()
        } else {
            super::card::front(&self.game, 380., None, false, cx).into_any_element()
        };
        let entity = cx.entity();
        v_flex()
            .id("proof")
            .track_focus(&self.focus)
            .size_full()
            .items_center()
            .justify_center()
            .gap_6()
            .bg(rgb(0xadb3bf))
            .text_color(rgb(0x20242c))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "space" => {
                        this.back = !this.back;
                        this.turn
                            .set(if this.back { std::f32::consts::PI } else { 0. });
                    }
                    "1" => this.turn = Spring::new(0.6),
                    "2" => this.turn = Spring::new(std::f32::consts::FRAC_PI_2),
                    "3" => this.turn = Spring::new(std::f32::consts::PI),
                    "q" => cx.quit(),
                    _ => (),
                };
                cx.notify();
            }))
            .child(
                div()
                    .id("proof-card")
                    .relative()
                    .w(px(380.))
                    .h(px(554.8))
                    .child(card_layer(
                        1,
                        size(px(380.), px(554.8)),
                        CardPose {
                            pitch: self.pitch.value(),
                            yaw: turn + self.yaw.value(),
                            back,
                            material: true,
                            frosted_top: 0.,
                        },
                        crate::theme::interface_radius(cx, px(17.)),
                        face,
                    ))
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                entity.update(cx, |this, _| this.hover_bounds = bounds);
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        if this.turn.active() {
                            return;
                        }
                        let bounds = this.hover_bounds;
                        let x = f32::from(event.position.x - bounds.origin.x)
                            / f32::from(bounds.size.width).max(1.);
                        let y = f32::from(event.position.y - bounds.origin.y)
                            / f32::from(bounds.size.height).max(1.);
                        this.pitch
                            .set((0.5 - y.clamp(0., 1.)) * 10_f32.to_radians());
                        this.yaw.set((x.clamp(0., 1.) - 0.5) * 16_f32.to_radians());
                        cx.notify();
                    }))
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        if !hovered {
                            this.pitch.set(0.);
                            this.yaw.set(0.);
                            cx.notify();
                        }
                    })),
            )
            .child("Space: turn · 1: perspective · 2: edge · 3: rear · Q: quit")
    }
}
pub fn run() -> anyhow::Result<()> {
    gpui::set_linux_card_compositor_enabled(true);
    #[cfg(target_os = "linux")]
    anyhow::ensure!(
        std::env::var_os("DISPLAY").is_some()
            || (std::env::var_os("WAYLAND_DISPLAY").is_some()
                && std::env::var_os("XDG_RUNTIME_DIR").is_some()),
        "GameSync card proof needs a graphical session. Start it from an Omarchy terminal, or set XDG_RUNTIME_DIR and WAYLAND_DISPLAY to the active Wayland session."
    );
    let game = crate::fixtures::games()?.into_iter().next().unwrap();
    Application::new()
        .with_assets(crate::assets::Assets)
        .run(move |cx| {
            gpui_component::init(cx);
            cx.activate(true);
            let result = cx.open_window(
                WindowOptions {
                    app_id: Some(crate::APP_ID.into()),
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(800.), px(760.)),
                        cx,
                    ))),
                    ..Default::default()
                },
                |window, cx| {
                    let proof = cx.new(|cx| Proof {
                        game,
                        turn: Spring::new(0.6),
                        pitch: Spring::new(0.),
                        yaw: Spring::new(0.),
                        hover_bounds: Bounds::default(),
                        back: false,
                        focus: cx.focus_handle(),
                    });
                    window.focus(&proof.read(cx).focus);
                    cx.new(|cx| Root::new(proof, window, cx))
                },
            );
            if let Err(error) = result {
                log::error!("Could not open GameSync card proof: {error}");
                cx.quit();
            }
        });
    Ok(())
}
