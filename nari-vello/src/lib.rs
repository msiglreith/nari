mod engine;
mod fxp;

pub mod icon;
pub mod typo;
use parley::fontique::Attributes;
pub use vello::*;

use self::{
    engine::Engine,
    kurbo::{Affine, Point},
    peniko::{Brush, Color, Fill},
    typo::{Font, FontScaled, FontSize, GlyphCache, GlyphKey, TextRun},
};
use nari_platform::{Extent, Surface};

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
    swapchain: wgpu::Surface<'static>,
    swapchain_config: wgpu::SurfaceConfiguration,
    engine: Engine,
    renderer: vello::Renderer,
    glyph_cache: typo::GlyphCache,
    scale: f64,
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
                    required_features: features
                        & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::CLEAR_TEXTURE),
                    required_limits: limits,
                },
                None,
            )
            .await
            .unwrap();

        let swapchain = instance.create_surface(surface).unwrap();
        let size = surface.extent();
        let swapchain_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width as _,
            height: size.height as _,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 3,
        };
        swapchain.configure(&device, &swapchain_config);

        let renderer = vello::Renderer::new(
            &device,
            RendererOptions {
                surface_format: Some(wgpu::TextureFormat::Bgra8UnormSrgb),
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                num_init_threads: None,
            },
        )
        .unwrap();
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
            scale: surface.dpi(),
        }
    }

    pub fn present(&mut self, scene: &Scene, background: Color) {
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
                &RenderParams {
                    base_color: background,
                    width: self.swapchain_config.width,
                    height: self.swapchain_config.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .expect("failed to render to surface");

        frame_image.present();
        self.device.poll(wgpu::Maintain::Poll);
    }

    pub fn resize(&mut self, size: Extent) {
        self.swapchain_config.width = size.width as _;
        self.swapchain_config.height = size.height as _;
        self.swapchain
            .configure(&self.device, &self.swapchain_config);
    }

    pub fn scale(&self, x: f64) -> f64 {
        (x * self.scale).round()
    }

    pub fn scale_pt(&self, pt: Point) -> Point {
        Point::new(self.scale(pt.x), self.scale(pt.y))
    }

    pub fn create_font(&mut self, data: Vec<u8>) -> Font {
        self.engine.create_font(data)
    }

    pub fn create_font_scaled(
        &mut self,
        font: Font,
        size: FontSize,
        attributes: Attributes,
    ) -> FontScaled {
        self.engine.create_font_scaled(font, size, attributes)
    }

    pub fn build_text_run<S: AsRef<str>>(&mut self, font: FontScaled, text: S) -> TextRun {
        self.engine
            .build_text_run(font, text, &mut self.glyph_cache)
    }

    pub fn text_run(
        &self,
        sb: &mut Scene,
        text_run: &TextRun,
        affine: Affine,
        align_x: Align,
        brush: Brush,
    ) {
        let transform = affine * Affine::scale_non_uniform(1.0, -1.0);
        let px: f64 = text_run.offset_x(align_x);

        for line in text_run.layout.lines() {
            for glyph_run in line.glyph_runs() {
                let mut run_x = px as f32 + glyph_run.offset();
                let run_y = glyph_run.baseline();

                for glyph in glyph_run.glyphs() {
                    let glyph_x = run_x + glyph.x;
                    let glyph_y = run_y - glyph.y;
                    run_x += glyph.advance;

                    let key = GlyphKey { id: glyph.id as _ };
                    let path = self
                        .glyph_cache
                        .get(&(text_run.font.size, key))
                        .expect("missing glyph entry");

                    sb.fill(
                        Fill::NonZero,
                        Affine::translate((glyph_x as _, glyph_y as _)) * transform,
                        &brush,
                        None,
                        &path,
                    );
                }
            }
        }
    }
}
