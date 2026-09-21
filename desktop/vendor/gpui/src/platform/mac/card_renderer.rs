//! GameSync's bounded, opt-in offscreen card compositor. Metal retains in-flight resources.
use super::*;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};
const BUDGET: usize = 64 * 1024 * 1024;
fn allocation_size(surface: &PaintSurface) -> (u64, u64) {
    let bucket = |value: f32| (value.ceil().max(1.) as u64).div_ceil(64) * 64;
    (
        bucket(surface.bounds.size.width.0),
        bucket(surface.bounds.size.height.0),
    )
}

struct CachedFace {
    texture: metal::Texture,
    content: Arc<crate::card_layer::CardLayer>,
    used: u64,
    valid: bool,
}
impl CachedFace {
    fn bytes(&self) -> usize {
        self.texture.allocated_size() as usize
    }
}

pub(super) struct CardRenderer {
    pipeline: metal::RenderPipelineState,
    faces: HashMap<u64, CachedFace>,
    retired: Vec<CachedFace>,
    scratch: Option<CachedFace>,
    frame: u64,
    completed: Arc<AtomicU64>,
    started: Instant,
    trace: bool,
    renders: usize,
    hits: usize,
    peak_bytes: usize,
}
impl CardRenderer {
    pub(super) fn new(device: &metal::DeviceRef) -> Self {
        let library = device
            .new_library_with_source(
                include_str!("card_shaders.metal"),
                &metal::CompileOptions::new(),
            )
            .expect("GameSync card shader must compile");
        let descriptor = metal::RenderPipelineDescriptor::new();
        descriptor.set_vertex_function(Some(&library.get_function("card_vertex", None).unwrap()));
        descriptor
            .set_fragment_function(Some(&library.get_function("card_fragment", None).unwrap()));
        let color = descriptor.color_attachments().object_at(0).unwrap();
        color.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        color.set_blending_enabled(true);
        color.set_source_rgb_blend_factor(metal::MTLBlendFactor::One);
        color.set_destination_rgb_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
        color.set_source_alpha_blend_factor(metal::MTLBlendFactor::One);
        color.set_destination_alpha_blend_factor(metal::MTLBlendFactor::OneMinusSourceAlpha);
        Self {
            pipeline: device.new_render_pipeline_state(&descriptor).unwrap(),
            faces: HashMap::new(),
            scratch: None,
            retired: Vec::new(),
            frame: 0,
            completed: Arc::new(AtomicU64::new(0)),
            started: Instant::now(),
            trace: std::env::var_os("GAMESYNC_CARD_TRACE").is_some(),
            renders: 0,
            hits: 0,
            peak_bytes: 0,
        }
    }
    pub(super) fn begin_frame(&mut self) {
        self.frame += 1;
        self.started = Instant::now();
        self.renders = 0;
        self.hits = 0;
        let completed = self.completed.load(Ordering::Acquire);
        self.retired.retain(|face| face.used > completed);
        if self
            .scratch
            .as_ref()
            .is_some_and(|face| face.used + 2 < self.frame)
        {
            let face = self.scratch.take().unwrap();
            if face.used > completed {
                self.retired.push(face);
            }
        }
        // Hidden cards do not accumulate textures while browsing a large library.
        let stale: Vec<_> = self
            .faces
            .iter()
            .filter(|(_, f)| f.used + 2 < self.frame)
            .map(|(&id, _)| id)
            .collect();
        for id in stale {
            self.retire(id);
        }
    }
    fn retire(&mut self, id: u64) {
        if let Some(face) = self.faces.remove(&id) {
            if face.used > self.completed.load(Ordering::Acquire) {
                self.retired.push(face);
            }
        }
    }
    fn bytes(&self) -> usize {
        self.faces.values().map(CachedFace::bytes).sum::<usize>()
            + self.scratch.as_ref().map_or(0, CachedFace::bytes)
            + self.retired.iter().map(CachedFace::bytes).sum::<usize>()
    }
    pub(super) fn cached(&mut self, surface: &PaintSurface) -> bool {
        let card = surface.card.as_ref().unwrap();
        if !card.pose.material {
            return false;
        }
        if let Some(face) = self.faces.get_mut(&card.id) {
            let (width, height) = allocation_size(surface);
            let same = face.valid
                && face.texture.width() == width
                && face.texture.height() == height
                && face.content.scene.paint_operations == card.scene.paint_operations;
            if same {
                face.used = self.frame;
                self.hits += 1;
                return true;
            }
        }
        false
    }
    pub(super) fn allocate(
        &mut self,
        surface: &PaintSurface,
        device: &metal::DeviceRef,
    ) -> Option<metal::Texture> {
        let id = surface.card.as_ref()?.id;
        let (width, height) = allocation_size(surface);
        let transient = !surface.card.as_ref()?.pose.material;
        if transient {
            if let Some(face) = &self.scratch {
                if face.texture.width() == width && face.texture.height() == height {
                    return Some(face.texture.clone());
                }
            }
            if let Some(face) = self.scratch.take() {
                if face.used > self.completed.load(Ordering::Acquire) {
                    self.retired.push(face);
                }
            }
        }
        if let Some(face) = self.faces.get(&id) {
            if face.texture.width() == width && face.texture.height() == height {
                return Some(face.texture.clone());
            }
        }
        self.retire(id);
        let descriptor = metal::TextureDescriptor::new();
        descriptor.set_width(width);
        descriptor.set_height(height);
        descriptor.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        descriptor.set_storage_mode(metal::MTLStorageMode::Private);
        descriptor
            .set_usage(metal::MTLTextureUsage::RenderTarget | metal::MTLTextureUsage::ShaderRead);
        let cost = device.heap_texture_size_and_align(&descriptor).size as usize;
        while self.bytes() + cost > BUDGET {
            let oldest = self
                .faces
                .iter()
                .filter(|(_, f)| f.used < self.frame)
                .min_by_key(|(_, f)| f.used)
                .map(|(&id, _)| id);
            if let Some(oldest) = oldest {
                self.retire(oldest);
            } else {
                log::error!(
                    "Card texture budget exhausted: {} + {} bytes",
                    self.bytes(),
                    cost
                );
                return None;
            }
        }
        Some(device.new_texture(&descriptor))
    }
    pub(super) fn store(&mut self, surface: &PaintSurface, texture: metal::Texture) {
        let card = surface.card.as_ref().unwrap();
        let face = CachedFace {
            texture,
            content: card.clone(),
            used: self.frame,
            valid: true,
        };
        if card.pose.material {
            self.faces.insert(card.id, face);
        } else {
            self.scratch = Some(face);
        }
        self.peak_bytes = self.peak_bytes.max(self.bytes());
        self.renders += 1;
    }
    pub(super) fn draw(
        &self,
        surface: &PaintSurface,
        viewport: Size<DevicePixels>,
        encoder: &metal::RenderCommandEncoderRef,
    ) {
        let card = surface.card.as_ref().unwrap();
        let Some(face) = (if card.pose.material {
            self.faces.get(&card.id)
        } else {
            self.scratch.as_ref()
        }) else {
            return;
        };
        let b = surface.bounds;
        let clip = surface.content_mask.bounds;
        let mut uniforms = [
            b.origin.x.0,
            b.origin.y.0,
            b.size.width.0,
            b.size.height.0,
            viewport.width.0 as f32,
            viewport.height.0 as f32,
            card.pose.pitch,
            card.pose.yaw,
            card.radius,
            if card.pose.back { 1. } else { 0. },
            if card.pose.material && card.pose.frosted_top == 0. {
                1.
            } else {
                0.
            },
            0.,
            clip.origin.x.0,
            clip.origin.y.0,
            clip.size.width.0,
            clip.size.height.0,
            b.size.width.0 / face.texture.width() as f32,
            b.size.height.0 / face.texture.height() as f32,
            card.pose.frosted_top,
            0.,
        ];
        encoder.set_render_pipeline_state(&self.pipeline);
        encoder.set_fragment_texture(0, Some(&face.texture));
        let modes: &[f32] = if card.pose.material && card.pose.frosted_top == 0. {
            &[2., 1., 0.]
        } else {
            &[0.]
        };
        for &mode in modes {
            uniforms[11] = mode;
            encoder.set_vertex_bytes(
                0,
                mem::size_of_val(&uniforms) as u64,
                uniforms.as_ptr().cast(),
            );
            encoder.set_fragment_bytes(
                0,
                mem::size_of_val(&uniforms) as u64,
                uniforms.as_ptr().cast(),
            );
            encoder.draw_primitives(metal::MTLPrimitiveType::Triangle, 0, 6);
        }
    }
    pub(super) fn discard_unsubmitted(&mut self) {
        // The command failed before submission; none of this frame's rasterizations are valid.
        for face in self.faces.values_mut() {
            face.valid = false;
        }
        if let Some(face) = &mut self.scratch {
            face.valid = false;
        }
    }
    pub(super) fn on_submit(&self, command: &metal::CommandBufferRef) {
        let completed = self.completed.clone();
        let frame = self.frame;
        let trace = self.trace && (frame < 8 || frame % 30 == 0);
        command.add_completed_handler(
            &ConcreteBlock::new(move |buffer: &metal::CommandBufferRef| {
                completed.fetch_max(frame, Ordering::Release);
                if trace {
                    let start: f64 = unsafe { msg_send![buffer, GPUStartTime] };
                    let end: f64 = unsafe { msg_send![buffer, GPUEndTime] };
                    eprintln!(
                        "[card-layer-gpu] frame={} gpu_us={:.0}",
                        frame,
                        (end - start) * 1_000_000.
                    );
                }
            })
            .copy(),
        );
        if self.trace && (self.frame < 8 || self.frame % 30 == 0) {
            eprintln!(
                "[card-layer] frame={} encode_us={} textures={} bytes={} peak_bytes={} rasterized={} cache_hits={}",
                self.frame,
                self.started.elapsed().as_micros(),
                self.faces.len(),
                self.bytes(),
                self.peak_bytes,
                self.renders,
                self.hits
            );
        }
    }
}
