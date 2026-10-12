//! Renderer: alles, was mit der Grafikkarte (wgpu) spricht.
//!
//! Kein Wissen über Fenster-Bibliotheken – der Renderer bekommt nur eine
//! `wgpu::SurfaceTarget`. So kann dieselbe Datei später nativ und im Browser laufen.

/// Hintergrundfarbe (dunkles Holz, passend zu Grove).
const HINTERGRUND: wgpu::Color = wgpu::Color { r: 0.07, g: 0.05, b: 0.04, a: 1.0 };

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    /// Baut den Renderer für ein Fenster. `breite`/`hoehe` in Pixeln.
    pub async fn new(
        ziel: impl Into<wgpu::SurfaceTarget<'static>>,
        breite: u32,
        hoehe: u32,
    ) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance.create_surface(ziel).expect("Surface konnte nicht erstellt werden");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .expect("Keine passende Grafikkarte gefunden");
        let info = adapter.get_info();
        log::info!("Grafikkarte: {} ({:?})", info.name, info.backend);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Timbre Device"),
                ..Default::default()
            })
            .await
            .expect("Device konnte nicht erstellt werden");

        let mut config = surface
            .get_default_config(&adapter, breite.max(1), hoehe.max(1))
            .expect("Surface wird von dieser Grafikkarte nicht unterstützt");
        // sRGB-Format bevorzugen, damit Farben korrekt aussehen.
        let caps = surface.get_capabilities(&adapter);
        if let Some(srgb) = caps.formats.iter().copied().find(|f| f.is_srgb()) {
            config.format = srgb;
        }
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/dreieck.wgsl"));
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Dreieck Layout"),
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Dreieck Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self { surface, device, queue, config, pipeline }
    }

    /// Fenstergröße hat sich geändert.
    pub fn resize(&mut self, breite: u32, hoehe: u32) {
        if breite == 0 || hoehe == 0 {
            return; // minimiert
        }
        self.config.width = breite;
        self.config.height = hoehe;
        self.surface.configure(&self.device, &self.config);
    }

    /// Einen Frame zeichnen.
    pub fn render(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => f,
            wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                // Zeichnen geht noch, aber beim nächsten Frame neu konfigurieren.
                self.surface.configure(&self.device, &self.config);
                f
            }
            _ => {
                // Fenster verdeckt, Timeout oder Surface veraltet: Frame auslassen.
                self.surface.configure(&self.device, &self.config);
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Frame") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Hauptpass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(HINTERGRUND),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(frame);
    }
}
