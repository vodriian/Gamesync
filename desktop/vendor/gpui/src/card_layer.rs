//! GameSync's opt-in Metal card surface. Other renderers keep the original flat element.
use crate::{Pixels, Size, prelude::*};

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
    pub frosted_top: f32,
}

#[cfg(target_os = "macos")]
pub(crate) struct CardLayer {
    pub id: u64,
    pub scene: crate::Scene,
    pub pose: CardPose,
    pub radius: f32,
    pub scale_factor: f32,
}
#[cfg(target_os = "macos")]
impl std::fmt::Debug for CardLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CardLayer").field("id", &self.id).finish()
    }
}
#[cfg(target_os = "macos")]
impl PartialEq for CardLayer {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.pose == other.pose
            && self.radius == other.radius
            && self.scene.paint_operations == other.scene.paint_operations
    }
}

/// Composite a fixed-size native face into a perspective surface on macOS Metal.
/// Pose changes do not change layout or invalidate the cached face pixels.
pub fn card_layer(
    id: u64,
    dimensions: Size<Pixels>,
    pose: CardPose,
    radius: Pixels,
    element: impl IntoElement,
) -> crate::AnyElement {
    let element = element.into_any_element();
    #[cfg(all(target_os = "macos", not(feature = "macos-blade")))]
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
    #[cfg(not(all(target_os = "macos", not(feature = "macos-blade"))))]
    {
        let _ = (id, dimensions, pose, radius);
        element
    }
}
