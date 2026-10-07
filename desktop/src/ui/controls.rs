//! Settings controls that follow the platform's design language.
//!
//! On macOS the Settings window is a form in every look, so its controls use the
//! Apple UI kit's regular content-area metrics: a fill track with an
//! accent-filled selected segment, a capsule switch with a white knob, and a
//! filled pop-up bezel, all with 6 pt control corners. Theme only changes their
//! colors. Other platforms keep the separate tinted buttons from the Elyx design.
use gpui::{div, prelude::*, px, App, ClickEvent, ElementId, SharedString, Window};
use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariants as _},
    h_flex, ActiveTheme as _, Disableable as _, Icon, IconName, Selectable as _, Sizable as _,
};

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// macOS Settings uses the toolbar-tab form and kit controls in every look.
pub const MAC_SETTINGS: bool = cfg!(target_os = "macos");

pub struct Segment {
    pub label: SharedString,
    pub selected: bool,
    pub on_click: Handler,
}

impl Segment {
    pub fn new(
        label: impl Into<SharedString>,
        selected: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            selected,
            on_click: Box::new(on_click),
        }
    }
}

/// One choice from a short list of mutually exclusive options.
pub fn segmented(
    id: &'static str,
    segments: Vec<Segment>,
    disabled: bool,
    cx: &App,
) -> gpui::AnyElement {
    let theme = cx.theme();
    if !MAC_SETTINGS {
        return h_flex()
            .gap_2()
            .children(segments.into_iter().enumerate().map(|(index, segment)| {
                Button::new(ElementId::NamedInteger(id.into(), index as u64))
                    .label(segment.label)
                    .selected(segment.selected)
                    .disabled(disabled)
                    .on_click(segment.on_click)
            }))
            .into_any_element();
    }
    let selected = ButtonCustomVariant::new(cx)
        .color(theme.primary)
        .foreground(theme.primary_foreground)
        .border(theme.transparent)
        .hover(theme.primary_hover)
        .active(theme.primary_active);
    let normal = ButtonCustomVariant::new(cx)
        .color(theme.transparent)
        .foreground(theme.foreground)
        .border(theme.transparent)
        .hover(theme.secondary_hover)
        .active(theme.secondary_active);
    h_flex()
        .rounded(px(6.))
        .bg(theme.secondary)
        .children(segments.into_iter().enumerate().map(|(index, segment)| {
            Button::new(ElementId::NamedInteger(id.into(), index as u64))
                .small()
                .px_3()
                .rounded(px(6.))
                .label(segment.label)
                .custom(if segment.selected { selected } else { normal })
                .disabled(disabled)
                .on_click(segment.on_click)
        }))
        .into_any_element()
}

/// Off track for native switches. The kit's 10% fill disappears on grouped
/// surfaces, so derive it from secondary text for visible contrast.
pub fn switch_off(cx: &App) -> gpui::Hsla {
    cx.theme().muted_foreground.opacity(0.35)
}

/// A boolean setting. Changes apply immediately, with no thumb animation.
pub fn switch(id: impl Into<ElementId>, on: bool, disabled: bool, cx: &App) -> Button {
    let theme = cx.theme();
    // Kit small switch: 44 x 20 capsule with a 2 pt inset knob.
    let (width, height, knob) = if MAC_SETTINGS {
        (44., 20., 26.)
    } else {
        (36., 20., 16.)
    };
    let inset = 2.;
    // A white knob reads on both the accent and the off track in every scheme.
    let (track, thumb) = if on {
        (theme.primary, gpui::white())
    } else {
        (switch_off(cx), gpui::white())
    };
    Button::new(id)
        .ghost()
        .w(px(width + 8.))
        .h(px(height + 8.))
        .p_0()
        .disabled(disabled)
        .child(
            div()
                .relative()
                .w(px(width))
                .h(px(height))
                .rounded(px(height / 2.))
                .bg(track)
                .when(disabled, |track| track.opacity(0.5))
                .child(
                    div()
                        .absolute()
                        .top(px(inset))
                        .left(px(if on { width - inset - knob } else { inset }))
                        .w(px(knob))
                        .h(px(height - 2. * inset))
                        .rounded(px((height - 2. * inset) / 2.))
                        .bg(thumb)
                        .shadow_sm(),
                ),
        )
}

/// Settings group background. Native macOS groups sit on a quiet raised fill,
/// not on the control bezel color.
pub fn group_surface(cx: &App) -> gpui::Hsla {
    if crate::theme::macos_shell(cx) {
        cx.theme().muted
    } else {
        cx.theme().secondary
    }
}

/// Width of the trailing-aligned label column in native macOS settings forms.
const FORM_LABEL: f32 = 180.;

/// One row of a macOS settings form: "Label:" right-aligned, then the control.
pub fn form_row(label: &'static str, control: impl IntoElement) -> gpui::Div {
    h_flex()
        .gap_3()
        .items_center()
        .child(
            div()
                .w(px(FORM_LABEL))
                .flex_shrink_0()
                .text_right()
                .child(label),
        )
        .child(h_flex().flex_1().min_w_0().child(control))
}

/// A divider between groups of form rows, inset like the controls column.
pub fn form_separator(cx: &App) -> gpui::Div {
    div().my_2().mx_8().h(px(1.)).bg(cx.theme().border)
}

/// A labeled row with a trailing switch.
pub fn switch_row(label: impl Into<SharedString>, switch: Button) -> gpui::Div {
    h_flex()
        .w_full()
        .min_h(px(32.))
        .justify_between()
        .gap_4()
        .child(div().child(label.into()))
        .child(switch)
}

/// A pop-up button face. Attach the menu with `DropdownMenu::dropdown_menu`.
pub fn popup(id: &'static str, value: impl Into<SharedString>, disabled: bool, cx: &App) -> Button {
    let theme = cx.theme();
    let value_label: SharedString = value.into();
    let face = h_flex()
        .w_full()
        .justify_between()
        .gap_2()
        .child(div().truncate().child(value_label.clone()))
        .child(Icon::new(IconName::ChevronDown).size_4());
    if MAC_SETTINGS {
        // macOS pop-up buttons size to their content and show up/down chevrons.
        let face = h_flex()
            .gap_2()
            .child(div().truncate().child(value_label))
            .child(Icon::new(IconName::ChevronsUpDown).size_3());
        return Button::new(id)
            .small()
            .disabled(disabled)
            .child(face)
            .rounded(px(6.))
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(theme.secondary)
                    .foreground(theme.foreground)
                    .border(theme.transparent)
                    .hover(theme.secondary_hover)
                    .active(theme.secondary_active),
            );
    }
    Button::new(id)
        .w_full()
        .disabled(disabled)
        .child(face)
        .outline()
}
