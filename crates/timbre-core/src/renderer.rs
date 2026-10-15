//! Renderer: alles, was mit der Grafikkarte (wgpu) spricht.
//!
//! Kein Wissen über Fenster-Bibliotheken – der Renderer bekommt nur eine
//! `wgpu::SurfaceTarget`. So läuft dieselbe Datei nativ und im Browser.
//!
//! Gezeichnet werden Sprites mit **Batching**: Alle Sprites mit derselben Textur
//! werden in einen gemeinsamen Puffer geschrieben und mit einem einzigen
//! Zeichenaufruf ("Draw Call") an die Grafikkarte geschickt. Das ist der
//! Unterschied zwischen 100 und 100.000 Sprites pro Frame.

use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use wgpu::util::DeviceExt;

use crate::kamera::Kamera2d;

/// Hintergrundfarbe: Polarnacht-Blau (#0E1B2A, als lineare Farbe) – passend zum Eis-Stil des Studios.
const HINTERGRUND: wgpu::Color = wgpu::Color {
    r: 0.0044,
    g: 0.0109,
    b: 0.0232,
    a: 1.0,
};

/// Verweis auf eine Textur, die der Renderer verwaltet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TexturId(u32);

impl TexturId {
    #[cfg(test)]
    pub(crate) fn test(n: u32) -> Self {
        Self(n)
    }
}

/// Ein Bild in der Welt.
#[derive(Clone, Copy, Debug)]
pub struct Sprite {
    pub position: Vec2,
    pub groesse: Vec2,
    /// Drehung im Bogenmaß, gegen den Uhrzeigersinn.
    pub drehung: f32,
    /// Wird mit der Textur multipliziert (weiß = unverändert). RGBA, 0..1.
    pub farbe: [f32; 4],
    pub textur: TexturId,
    /// Ausschnitt der Textur: x, y, Breite, Höhe (0..1). Standard: ganze Textur.
    pub uv: [f32; 4],
    /// Zeichenreihenfolge: höhere Ebenen liegen oben.
    pub ebene: i32,
}

impl Sprite {
    pub fn new(textur: TexturId, position: Vec2, groesse: Vec2) -> Self {
        Self {
            position,
            groesse,
            drehung: 0.0,
            farbe: [1.0; 4],
            textur,
            uv: [0.0, 0.0, 1.0, 1.0],
            ebene: 0,
        }
    }
}

/// So liegt ein Sprite im GPU-Puffer (muss zu `Instanz` in sprite.wgsl passen).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SpriteInstanz {
    position: [f32; 2],
    groesse: [f32; 2],
    drehung: f32,
    farbe: [f32; 4],
    uv: [f32; 4],
}

/// Rechteck auf der Zeichenfläche in Pixeln (links oben = 0,0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bereich {
    pub x: f32,
    pub y: f32,
    pub breite: f32,
    pub hoehe: f32,
}

impl Bereich {
    pub fn groesse(&self) -> Vec2 {
        Vec2::new(self.breite, self.hoehe)
    }
}

/// Ein Frame in Arbeit: zuerst Sprites, danach z. B. die Editor-Oberfläche, dann anzeigen.
pub struct Frame {
    textur: wgpu::SurfaceTexture,
    pub view: wgpu::TextureView,
    /// Dieselbe Fläche ohne sRGB-Umrechnung (für Oberflächen wie egui).
    pub view_linear: wgpu::TextureView,
    pub encoder: wgpu::CommandEncoder,
}

/// Zahlen zum letzten Frame (für die Anzeige).
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameStatistik {
    pub sprites: usize,
    pub draw_calls: usize,
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    kamera_puffer: wgpu::Buffer,
    kamera_gruppe: wgpu::BindGroup,
    textur_layout: wgpu::BindGroupLayout,
    abtaster: wgpu::Sampler,
    texturen: Vec<wgpu::BindGroup>,
    instanz_puffer: wgpu::Buffer,
    instanz_kapazitaet: usize,
    // Wiederverwendete Arbeitslisten, damit nicht jeden Frame neu Speicher angefordert wird.
    sortiert: Vec<((i32, TexturId), SpriteInstanz)>,
    instanzen: Vec<SpriteInstanz>,
}

impl Renderer {
    /// Baut den Renderer für ein Fenster. `breite`/`hoehe` in Pixeln.
    pub async fn new(ziel: impl Into<wgpu::SurfaceTarget<'static>>, breite: u32, hoehe: u32) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let surface = instance
            .create_surface(ziel)
            .expect("Surface konnte nicht erstellt werden");

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
        // Zusätzlich eine Ansicht ohne sRGB erlauben – die braucht die Editor-Oberfläche (egui).
        config.view_formats = vec![config.format.remove_srgb_suffix()];
        surface.configure(&device, &config);

        // --- Kamera (Gruppe 0) ---
        let kamera_puffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Kamera"),
            size: std::mem::size_of::<[[f32; 4]; 4]>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let kamera_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Kamera Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let kamera_gruppe = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Kamera"),
            layout: &kamera_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: kamera_puffer.as_entire_binding(),
            }],
        });

        // --- Textur (Gruppe 1) ---
        let textur_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Textur Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let abtaster = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Linear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // --- Pipeline ---
        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/sprite.wgsl"));
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sprite Layout"),
            bind_group_layouts: &[Some(&kamera_layout), Some(&textur_layout)],
            ..Default::default()
        });
        let attribute = wgpu::vertex_attr_array![
            0 => Float32x2, // position
            1 => Float32x2, // groesse
            2 => Float32,   // drehung
            3 => Float32x4, // farbe
            4 => Float32x4, // uv
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sprite Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SpriteInstanz>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attribute,
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        let instanz_kapazitaet = 1024;
        let instanz_puffer = Self::instanz_puffer_bauen(&device, instanz_kapazitaet);

        Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            kamera_puffer,
            kamera_gruppe,
            textur_layout,
            abtaster,
            texturen: Vec::new(),
            instanz_puffer,
            instanz_kapazitaet,
            sortiert: Vec::new(),
            instanzen: Vec::new(),
        }
    }

    fn instanz_puffer_bauen(device: &wgpu::Device, kapazitaet: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Sprite-Instanzen"),
            size: (kapazitaet * std::mem::size_of::<SpriteInstanz>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Lädt ein Bild (RGBA, 4 Bytes pro Pixel, Zeile für Zeile von oben) auf die Grafikkarte.
    pub fn textur_laden(&mut self, breite: u32, hoehe: u32, rgba: &[u8]) -> TexturId {
        assert_eq!(
            rgba.len(),
            (breite * hoehe * 4) as usize,
            "RGBA-Daten passen nicht zur Größe"
        );
        let textur = self.device.create_texture_with_data(
            &self.queue,
            &wgpu::TextureDescriptor {
                label: Some("Sprite-Textur"),
                size: wgpu::Extent3d {
                    width: breite,
                    height: hoehe,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            rgba,
        );
        let view = textur.create_view(&wgpu::TextureViewDescriptor::default());
        let gruppe = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sprite-Textur"),
            layout: &self.textur_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.abtaster),
                },
            ],
        });
        self.texturen.push(gruppe);
        TexturId(self.texturen.len() as u32 - 1)
    }

    /// Lädt eine Textur aus dem `.tbr`-Format auf die Grafikkarte.
    pub fn textur_aus_tbr(&mut self, textur: &timbre_tbr::Textur) -> TexturId {
        log::info!(
            "Textur '{}' ({}×{}, ID {})",
            textur.name,
            textur.breite,
            textur.hoehe,
            textur.id
        );
        self.textur_laden(textur.breite, textur.hoehe, &textur.pixel)
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Pixelformat der Zeichenfläche.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    /// Pixelformat von [`Frame::view_linear`] (für die Editor-Oberfläche).
    pub fn format_linear(&self) -> wgpu::TextureFormat {
        self.config.format.remove_srgb_suffix()
    }

    /// Aktuelle Zeichenfläche in Pixeln.
    pub fn groesse(&self) -> Vec2 {
        Vec2::new(self.config.width as f32, self.config.height as f32)
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

    /// Einen Frame komplett zeichnen: Sprites auf der ganzen Fläche, dann anzeigen.
    pub fn zeichnen(&mut self, kamera: &Kamera2d, sprites: &[Sprite]) -> FrameStatistik {
        let Some(mut frame) = self.frame_beginnen() else {
            return FrameStatistik::default();
        };
        let g = self.groesse();
        let stat = self.sprites_zeichnen(
            &mut frame,
            kamera,
            sprites,
            Bereich {
                x: 0.0,
                y: 0.0,
                breite: g.x,
                hoehe: g.y,
            },
        );
        self.frame_beenden(frame);
        stat
    }

    /// Neuen Frame anfangen und mit der Hintergrundfarbe füllen.
    /// `None`, wenn gerade nicht gezeichnet werden kann (Fenster verdeckt o. Ä.).
    pub fn frame_beginnen(&mut self) -> Option<Frame> {
        let textur = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => f,
            wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                self.surface.configure(&self.device, &self.config);
                f
            }
            _ => {
                // Fenster verdeckt, Timeout oder Surface veraltet: Frame auslassen.
                self.surface.configure(&self.device, &self.config);
                return None;
            }
        };
        let view = textur.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Frame") });
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Hintergrund"),
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
        let view_linear = textur.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.format_linear()),
            ..Default::default()
        });
        Some(Frame {
            textur,
            view,
            view_linear,
            encoder,
        })
    }

    /// Frame an den Bildschirm geben.
    pub fn frame_beenden(&mut self, frame: Frame) {
        self.queue.submit([frame.encoder.finish()]);
        self.queue.present(frame.textur);
    }

    /// Sprites aus Sicht der Kamera in einen Bereich des Frames zeichnen.
    /// Nur einmal pro Frame aufrufen (der Sprite-Puffer wird dabei neu befüllt).
    pub fn sprites_zeichnen(
        &mut self,
        frame: &mut Frame,
        kamera: &Kamera2d,
        sprites: &[Sprite],
        bereich: Bereich,
    ) -> FrameStatistik {
        // Bereich auf die Zeichenfläche begrenzen (sonst meckert wgpu).
        let flaeche = self.groesse();
        let x = bereich.x.clamp(0.0, flaeche.x);
        let y = bereich.y.clamp(0.0, flaeche.y);
        let b = bereich.breite.min(flaeche.x - x);
        let h = bereich.hoehe.min(flaeche.y - y);
        if b < 1.0 || h < 1.0 {
            return FrameStatistik::default();
        }

        // 1. Kamera-Matrix hochladen
        let matrix = kamera.matrix(Vec2::new(b, h));
        self.queue
            .write_buffer(&self.kamera_puffer, 0, bytemuck::cast_slice(&matrix.to_cols_array()));
        // 2. Sprites nach (Ebene, Textur) sortieren: Ebenen bestimmen, was oben liegt,
        //    und innerhalb einer Ebene liegen gleiche Texturen hintereinander → wenige Draw Calls.
        //    Stabil sortieren, damit die Reihenfolge innerhalb einer Gruppe erhalten bleibt.
        self.sortiert.clear();
        self.sortiert.extend(sprites.iter().map(|s| {
            (
                (s.ebene, s.textur),
                SpriteInstanz {
                    position: s.position.to_array(),
                    groesse: s.groesse.to_array(),
                    drehung: s.drehung,
                    farbe: s.farbe,
                    uv: s.uv,
                },
            )
        }));
        self.sortiert.sort_by_key(|(t, _)| *t);
        self.instanzen.clear();
        self.instanzen.extend(self.sortiert.iter().map(|(_, i)| *i));

        // 3. Instanz-Puffer bei Bedarf vergrößern (verdoppeln) und befüllen.
        if self.instanzen.len() > self.instanz_kapazitaet {
            self.instanz_kapazitaet = self.instanzen.len().next_power_of_two();
            self.instanz_puffer = Self::instanz_puffer_bauen(&self.device, self.instanz_kapazitaet);
            log::info!("Sprite-Puffer vergrößert auf {} Plätze", self.instanz_kapazitaet);
        }
        if !self.instanzen.is_empty() {
            self.queue
                .write_buffer(&self.instanz_puffer, 0, bytemuck::cast_slice(&self.instanzen));
        }

        // 4. Zeichnen: ein Draw Call pro zusammenhängender Gruppe (gleiche Ebene + Textur).
        let mut draw_calls = 0;
        {
            let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Hauptpass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_viewport(x, y, b, h, 0.0, 1.0);
            pass.set_scissor_rect(x as u32, y as u32, b as u32, h as u32);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.kamera_gruppe, &[]);
            pass.set_vertex_buffer(0, self.instanz_puffer.slice(..));

            let mut start = 0;
            while start < self.sortiert.len() {
                let gruppe = self.sortiert[start].0;
                let mut ende = start;
                while ende < self.sortiert.len() && self.sortiert[ende].0 == gruppe {
                    ende += 1;
                }
                pass.set_bind_group(1, &self.texturen[gruppe.1 .0 as usize], &[]);
                pass.draw(0..6, start as u32..ende as u32);
                draw_calls += 1;
                start = ende;
            }
        }
        FrameStatistik {
            sprites: sprites.len(),
            draw_calls,
        }
    }
}
