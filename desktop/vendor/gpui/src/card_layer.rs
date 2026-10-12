//! GameSync's opt-in native card surface. Unsupported renderers keep the flat element.
use crate::{Pixels, Size, prelude::*};
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_os = "linux")]
static LINUX_CARD_COMPOSITOR_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable the experimental Linux compositor for Omarchy mode or its proof fixture.
#[doc(hidden)]
pub fn set_linux_card_compositor_enabled(enabled: bool) {
    #[cfg(target_os = "linux")]
    LINUX_CARD_COMPOSITOR_ENABLED.store(enabled, Ordering::Relaxed);
    #[cfg(not(target_os = "linux"))]
    let _ = enabled;
}

/// `GAMESYNC_FLAT_CARDS=1` keeps every card flat on every platform. It is a
/// diagnostic, and a fallback for a GPU that cannot composite cards.
fn flat_cards() -> bool {
    static FLAT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FLAT.get_or_init(|| std::env::var_os("GAMESYNC_FLAT_CARDS").is_some())
}

/// True when this renderer composites a material card layer in perspective:
/// Metal, DirectX, and Blade on Linux while its compositor is enabled.
pub fn card_compositor_available() -> bool {
    if flat_cards() {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        LINUX_CARD_COMPOSITOR_ENABLED.load(Ordering::Relaxed)
    }
    #[cfg(not(target_os = "linux"))]
    {
        cfg!(any(
            target_os = "windows",
            all(target_os = "macos", not(feature = "macos-blade"))
        ))
    }
}

/// True when a face can turn on an edge (`CardPose::hinge`). Blade keeps the
/// center axis.
pub fn card_hinge_supported() -> bool {
    card_compositor_available() && !cfg!(target_os = "linux")
}

/// True when a mask-only layer (`material: false`) is composited, which is
/// Metal only. Other renderers return the flat element; the caller rounds
/// that content itself.
pub fn card_mask_supported() -> bool {
    !flat_cards() && cfg!(all(target_os = "macos", not(feature = "macos-blade")))
}

/// Angles are radians. A back face is authored upright, then turned by PI before projection.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CardPose {
    /// Rotation around the horizontal axis.
    pub pitch: f32,
    /// Rotation around the vertical axis, including a full card turn.
    pub yaw: f32,
    /// Whether the captured face is the rear side of the card.
    pub back: bool,
    /// Enable satin light, rim, and projected shadow. False creates a rounded composite mask.
    pub material: bool,
    /// Fraction of the surface height to blur for an overlaid library toolbar.
    /// A positive value disables card lighting and uses the surface as flat chrome.
    /// Metal only; DirectX draws the surface without the blur.
    pub frosted_top: f32,
    /// Vertical rotation axis as a fraction of half the width: -1 is the left edge,
    /// 0 the center, and 1 the right edge. An edge axis turns the face like a book
    /// cover on its spine. Metal and DirectX; Blade keeps the center axis.
    pub hinge: f32,
    /// Fraction of the radius kept on the two corners at the hinge edge:
    /// 1 keeps them round, 0 makes them square. Ignored when `hinge` is 0.
    /// Hinged faces also skip the projected shadow; the caller draws one
    /// table shadow for the whole book.
    pub spine_round: f32,
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
pub(crate) struct CardLayer {
    pub id: u64,
    pub scene: crate::Scene,
    pub pose: CardPose,
    pub radius: f32,
    pub scale_factor: f32,
}
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
impl std::fmt::Debug for CardLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CardLayer").field("id", &self.id).finish()
    }
}
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
impl PartialEq for CardLayer {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.pose == other.pose
            && self.radius == other.radius
            && self.scene.paint_operations == other.scene.paint_operations
    }
}

/// Face texture size for a surface. 64-pixel buckets avoid a new allocation
/// for each fractional column width.
#[cfg(any(
    target_os = "windows",
    all(target_os = "macos", not(feature = "macos-blade"))
))]
pub(crate) fn card_allocation_size(surface: &crate::PaintSurface) -> (u64, u64) {
    let bucket = |value: f32| (value.ceil().max(1.) as u64).div_ceil(64) * 64;
    (
        bucket(surface.bounds.size.width.0),
        bucket(surface.bounds.size.height.0),
    )
}

/// The uniform block shared by the Metal and DirectX card shaders: six
/// `float4` values. Index 11 is the pass mode; set it from `card_passes`.
/// `viewport` and `texture` are sizes in device pixels.
#[cfg(any(
    target_os = "windows",
    all(target_os = "macos", not(feature = "macos-blade"))
))]
pub(crate) fn card_uniforms(
    surface: &crate::PaintSurface,
    card: &CardLayer,
    viewport: [f32; 2],
    texture: [f32; 2],
) -> [f32; 24] {
    let b = surface.bounds;
    let clip = surface.content_mask.bounds;
    [
        // rect
        b.origin.x.0,
        b.origin.y.0,
        b.size.width.0,
        b.size.height.0,
        // viewport_pose
        viewport[0],
        viewport[1],
        card.pose.pitch,
        card.pose.yaw,
        // shape
        card.radius,
        if card.pose.back { 1. } else { 0. },
        if card.pose.material && card.pose.frosted_top == 0. {
            1.
        } else {
            0.
        },
        0.,
        // clip
        clip.origin.x.0,
        clip.origin.y.0,
        clip.size.width.0,
        clip.size.height.0,
        // texture_region
        b.size.width.0 / texture[0],
        b.size.height.0 / texture[1],
        card.pose.frosted_top,
        card.scale_factor,
        // hinge
        card.pose.hinge,
        card.pose.spine_round,
        0.,
        0.,
    ]
}

/// Shader pass modes for one card, in draw order: 2 is the projected shadow,
/// 1 the thin edge, 0 the face. A hinged face is one page of a book; the app
/// draws a single table shadow for the whole book, so the page casts none.
#[cfg(any(
    target_os = "windows",
    all(target_os = "macos", not(feature = "macos-blade"))
))]
pub(crate) fn card_passes(pose: &CardPose) -> &'static [f32] {
    if pose.material && pose.frosted_top == 0. {
        if pose.hinge != 0. { &[1., 0.] } else { &[2., 1., 0.] }
    } else {
        &[0.]
    }
}

/// Composite a fixed-size native face into a perspective surface.
/// Pose changes do not change layout or invalidate the cached face pixels.
pub fn card_layer(
    id: u64,
    dimensions: Size<Pixels>,
    pose: CardPose,
    radius: Pixels,
    element: impl IntoElement,
) -> crate::AnyElement {
    let element = element.into_any_element();
    // Only Metal composites mask-only layers. Keeping them flat elsewhere
    // avoids recursive surfaces: a mask sits inside a material face.
    if !card_compositor_available() || (!pose.material && !card_mask_supported()) {
        return element;
    }
    #[cfg(any(
        target_os = "linux",
        target_os = "windows",
        all(target_os = "macos", not(feature = "macos-blade"))
    ))]
    {
        let mut element = element;
        crate::canvas(
            move |bounds, window, cx| {
                element.layout_as_root(dimensions.map(crate::AvailableSpace::Definite), window, cx);
                element.prepaint_at(bounds.origin, window, cx);
                element
            },
            move |bounds, mut element, window, cx| {
                window
                    .paint_card_layer(id, bounds, pose, radius, |window| element.paint(window, cx));
            },
        )
        .w(dimensions.width)
        .h(dimensions.height)
        .flex_shrink_0()
        .into_any_element()
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "windows",
        all(target_os = "macos", not(feature = "macos-blade"))
    )))]
    {
        let _ = (id, dimensions, pose, radius);
        element
    }
}
