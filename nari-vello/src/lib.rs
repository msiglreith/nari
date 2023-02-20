mod engine;
mod fxp;
pub mod typo;

pub use self::engine::Engine;
use self::typo::{Font, FontScaled, FontSize, GlyphCache, GlyphKey, TextRun};

use nari_platform::{Extent, Surface};
use std::pin::Pin;
use vello::{
    kurbo::Affine,
    peniko::{Brush, Fill},
    Scene, SceneBuilder,
};

#[derive(Debug, Clone, Copy)]
pub enum Align {
    Negative,
    Center,
    Positive,
}

pub struct Canvas {
    _instance: wgpu::Instance,
    device: wgpu::Device,
    queue: wgpu::Queue,
    swapchain: wgpu::Surface,
    swapchain_config: wgpu::SurfaceConfiguration,
    engine: Pin<Box<Engine>>,
    renderer: vello::Renderer,
    glyph_cache: typo::GlyphCache,
}

impl Canvas {
    pub async fn new(surface: Surface) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapter = instance.request_adapter(&Default::default()).await.unwrap();
        let features = adapter.features();
        let limits = wgpu::Limits::default();
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: features
                        & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::CLEAR_TEXTURE),
                    limits,
                },
                None,
            )
            .await
            .unwrap();

        let swapchain = unsafe { instance.create_surface(&surface).unwrap() };
        let size = surface.extent();
        let swapchain_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };
        swapchain.configure(&device, &swapchain_config);

        let renderer = vello::Renderer::new(&device).unwrap();
        let engine = Engine::new();
        let glyph_cache = GlyphCache::default();

        Self {
            _instance: instance,
            device,
            swapchain,
            swapchain_config,
            queue,
            engine,
            renderer,
            glyph_cache,
        }
    }

    pub fn present(&mut self, scene: &Scene) {
        let frame_image = self
            .swapchain
            .get_current_texture()
            .expect("failed to get surface texture");

        self.renderer
            .render_to_surface(
                &self.device,
                &self.queue,
                scene,
                &frame_image,
                self.swapchain_config.width,
                self.swapchain_config.height,
            )
            .expect("failed to render to surface");

        frame_image.present();
        self.device.poll(wgpu::Maintain::Poll);
    }

    pub fn resize(&mut self, size: Extent) {
        self.swapchain_config.width = size.width;
        self.swapchain_config.height = size.height;
        self.swapchain
            .configure(&self.device, &self.swapchain_config);
    }

    pub fn create_font(&mut self, data: Vec<u8>) -> Font {
        self.engine.create_font(data)
    }

    pub fn create_font_scaled(&mut self, font: Font, size: FontSize) -> FontScaled {
        self.engine.create_font_scaled(font, size)
    }

    pub fn build_text_run<S: AsRef<str>>(&mut self, font: FontScaled, text: S) -> TextRun {
        self.engine
            .build_text_run(font, text, &mut self.glyph_cache)
    }

    pub fn text_run(
        &self,
        sb: &mut SceneBuilder,
        text_run: &TextRun,
        affine: Affine,
        align_x: Align,
        brush: Brush,
    ) {
        let transform = affine * Affine::scale_non_uniform(1.0, -1.0);
        let px = text_run.offset_x(align_x);
        for cluster in &text_run.clusters {
            for glyph in &cluster.glyphs {
                let key = GlyphKey {
                    id: glyph.id,
                    offset: glyph.offset.fract(),
                };
                let path = self
                    .glyph_cache
                    .get(&(text_run.font.size, key))
                    .expect("missing glyph entry");
                let advance = px + glyph.offset.trunc().f64();

                sb.fill(
                    Fill::NonZero,
                    Affine::translate((advance as _, 0.0)) * transform,
                    &brush,
                    None,
                    &path,
                );
            }
        }
    }
}
