//! Isolated native renderer acceptance fixture: one real game, including title and footer.
use super::card_motion::Spring;
use gpui::{prelude::*, *};
use gpui_component::{v_flex, Root};
struct Proof {
    game: crate::model::Game,
    turn: Spring,
    back: bool,
    focus: FocusHandle,
}
impl Render for Proof {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.turn.active() {
            window.request_animation_frame();
        }
        let yaw = self.turn.value();
        let back = yaw > std::f32::consts::FRAC_PI_2;
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
            .child(card_layer(
                1,
                size(px(380.), px(554.8)),
                CardPose {
                    pitch: 0.,
                    yaw,
                    back,
                    material: true,
                    frosted_top: 0.,
                },
                px(17.),
                face,
            ))
            .child("Space: turn · 1: perspective · 2: edge · 3: rear · Q: quit")
    }
}
pub fn run() -> anyhow::Result<()> {
    let game = crate::fixtures::games()?.into_iter().next().unwrap();
    Application::new()
        .with_assets(crate::assets::Assets)
        .run(move |cx| {
            gpui_component::init(cx);
            cx.activate(true);
            cx.open_window(
                WindowOptions {
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
                        back: false,
                        focus: cx.focus_handle(),
                    });
                    window.focus(&proof.read(cx).focus);
                    cx.new(|cx| Root::new(proof, window, cx))
                },
            )
            .unwrap();
        });
    Ok(())
}
