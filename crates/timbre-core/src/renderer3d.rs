//! 3D-Zeichnen: einfache Körper (Würfel, Kugel, Ebene, Zylinder) mit Licht, Tiefe und Nebel.
//!
//! Läuft neben dem 2D-Sprite-Renderer auf derselben Zeichenfläche: Das Studio zeigt je nach
//! Ansicht die 2D-Szene (Sprites) oder die 3D-Szene (Meshes). Gleiches Prinzip wie bei Sprites:
//! Instancing – alle Objekte mit derselben Form landen in einem Zeichenaufruf.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::renderer::{Bereich, Frame, Renderer};

/// Die eingebauten Körper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Form3d {
    Wuerfel,
    Kugel,
    Ebene,
    Zylinder,
}

impl Form3d {
    pub const ALLE: [Form3d; 4] = [Form3d::Wuerfel, Form3d::Kugel, Form3d::Ebene, Form3d::Zylinder];

    pub fn name(self) -> &'static str {
        match self {
            Form3d::Wuerfel => "Würfel",
            Form3d::Kugel => "Kugel",
            Form3d::Ebene => "Ebene",
            Form3d::Zylinder => "Zylinder",
        }
    }

    /// Name in Szenen-Dateien und Scripts
    pub fn schluessel(self) -> &'static str {
        match self {
            Form3d::Wuerfel => "wuerfel",
            Form3d::Kugel => "kugel",
            Form3d::Ebene => "ebene",
            Form3d::Zylinder => "zylinder",
        }
    }

    pub fn aus_schluessel(s: &str) -> Option<Form3d> {
        Form3d::ALLE.into_iter().find(|f| {
            f.schluessel() == s
                || (s == "cube" && *f == Form3d::Wuerfel)
                || (s == "sphere" && *f == Form3d::Kugel)
                || (s == "plane" && *f == Form3d::Ebene)
                || (s == "cylinder" && *f == Form3d::Zylinder)
        })
    }

    /// Radius einer Kugel um den Körper (für Anklicken im Editor), bei Größe 1
    pub fn radius(self) -> f32 {
        match self {
            Form3d::Wuerfel => 0.87,
            Form3d::Kugel => 0.5,
            Form3d::Ebene => 0.71,
            Form3d::Zylinder => 0.71,
        }
    }
}

/// Ein Körper, der gezeichnet werden soll.
#[derive(Clone, Copy, Debug)]
pub struct Modell {
    pub form: Form3d,
    pub matrix: Mat4,
    pub farbe: [f32; 4],
    /// Ohne Licht und Nebel zeichnen (Gitterlinien, Markierungen im Editor)
    pub unbeleuchtet: bool,
}

/// Kamera, die um einen Punkt kreist (wie in Blender/Unity): Ziel, Abstand, zwei Winkel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kamera3d {
    pub ziel: Vec3,
    pub abstand: f32,
    /// Drehung um die senkrechte Achse (Bogenmaß)
    pub gier: f32,
    /// Neigung nach unten (Bogenmaß, 0 = waagerecht)
    pub neigung: f32,
    /// Öffnungswinkel senkrecht (Bogenmaß)
    pub sichtfeld: f32,
}

impl Default for Kamera3d {
    fn default() -> Self {
        Self {
            ziel: Vec3::ZERO,
            abstand: 12.0,
            gier: 0.7,
            neigung: 0.5,
            sichtfeld: 55f32.to_radians(),
        }
    }
}

impl Kamera3d {
    pub fn position(&self) -> Vec3 {
        let richtung = Vec3::new(
            self.neigung.cos() * self.gier.sin(),
            self.neigung.sin(),
            self.neigung.cos() * self.gier.cos(),
        );
        self.ziel + richtung * self.abstand
    }

    pub fn ansicht(&self) -> Mat4 {
        // Rechtshändig, Blick entlang -Z (von Hand, glam 0.34 hat die Projektions-API umgebaut)
        let auge = self.position();
        let f = (self.ziel - auge).normalize();
        let s = f.cross(Vec3::Y).normalize_or(Vec3::X);
        let u = s.cross(f);
        Mat4::from_cols(
            glam::vec4(s.x, u.x, -f.x, 0.0),
            glam::vec4(s.y, u.y, -f.y, 0.0),
            glam::vec4(s.z, u.z, -f.z, 0.0),
            glam::vec4(-s.dot(auge), -u.dot(auge), f.dot(auge), 1.0),
        )
    }

    pub fn projektion(&self, seitenverhaeltnis: f32) -> Mat4 {
        // Perspektive für wgpu (Tiefe 0..1)
        let (nah, fern) = (0.05f32, 2000.0f32);
        let h = 1.0 / (self.sichtfeld * 0.5).tan();
        let w = h / seitenverhaeltnis.max(0.01);
        let r = fern / (nah - fern);
        Mat4::from_cols(
            glam::vec4(w, 0.0, 0.0, 0.0),
            glam::vec4(0.0, h, 0.0, 0.0),
            glam::vec4(0.0, 0.0, r, -1.0),
            glam::vec4(0.0, 0.0, r * nah, 0.0),
        )
    }

    /// Ansicht × Projektion (Welt → Bildschirm)
    pub fn matrix(&self, seitenverhaeltnis: f32) -> Mat4 {
        self.projektion(seitenverhaeltnis) * self.ansicht()
    }

    /// Strahl durch einen Bildschirmpunkt (`p` in Pixeln ab links oben, `groesse` = Bereich in Pixeln).
    pub fn strahl(&self, p: glam::Vec2, groesse: glam::Vec2) -> (Vec3, Vec3) {
        let ndc = glam::Vec2::new(p.x / groesse.x * 2.0 - 1.0, 1.0 - p.y / groesse.y * 2.0);
        let inv = self.matrix(groesse.x / groesse.y).inverse();
        let nah = inv.project_point3(Vec3::new(ndc.x, ndc.y, 0.0));
        let fern = inv.project_point3(Vec3::new(ndc.x, ndc.y, 1.0));
        (nah, (fern - nah).normalize_or_zero())
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Ecke {
    position: [f32; 3],
    normale: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Instanz {
    matrix: [[f32; 4]; 4],
    farbe: [f32; 4],
    /// x = 1: unbeleuchtet
    extra: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SzenenDaten {
    ansicht_projektion: [[f32; 4]; 4],
    kamera: [f32; 4],
    licht: [f32; 4],
    nebel: [f32; 4],
}

struct GpuMesh {
    ecken: wgpu::Buffer,
    indizes: wgpu::Buffer,
    anzahl: u32,
}

pub struct Renderer3d {
    pipeline: wgpu::RenderPipeline,
    szenen_puffer: wgpu::Buffer,
    szenen_gruppe: wgpu::BindGroup,
    meshes: Vec<(Form3d, GpuMesh)>,
    instanz_puffer: wgpu::Buffer,
    instanz_kapazitaet: usize,
    tiefe: Option<(wgpu::TextureView, u32, u32)>,
    /// (unbeleuchtet, Form, Instanz) – Unbeleuchtetes (Gitter, halbdurchsichtig) kommt zuletzt
    sortiert: Vec<((bool, Form3d), Instanz)>,
}

const TIEFENFORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

impl Renderer3d {
    pub fn new(r: &Renderer) -> Self {
        let device = r.device();
        let szenen_puffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("3D-Szene"),
            size: std::mem::size_of::<SzenenDaten>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("3D-Szene Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let szenen_gruppe = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("3D-Szene"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: szenen_puffer.as_entire_binding(),
            }],
        });
        let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/mesh.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mesh Layout"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let ecken_attribute = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let instanz_attribute = wgpu::vertex_attr_array![
            2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4, 7 => Float32x4
        ];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mesh Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Ecke>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &ecken_attribute,
                    }),
                    Some(wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Instanz>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &instanz_attribute,
                    }),
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: r.format(),
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None, // Ebenen von beiden Seiten sichtbar
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: TIEFENFORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let meshes = Form3d::ALLE
            .into_iter()
            .map(|f| {
                let (ecken, indizes) = koerper(f);
                let mesh = GpuMesh {
                    ecken: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(f.name()),
                        contents: bytemuck::cast_slice(&ecken),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
                    indizes: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(f.name()),
                        contents: bytemuck::cast_slice(&indizes),
                        usage: wgpu::BufferUsages::INDEX,
                    }),
                    anzahl: indizes.len() as u32,
                };
                (f, mesh)
            })
            .collect();
        let instanz_kapazitaet = 256;
        Self {
            pipeline,
            szenen_puffer,
            szenen_gruppe,
            meshes,
            instanz_puffer: instanz_puffer(device, instanz_kapazitaet),
            instanz_kapazitaet,
            tiefe: None,
            sortiert: Vec::new(),
        }
    }

    /// Körper aus Sicht der Kamera in einen Bereich des Frames zeichnen.
    pub fn zeichnen(
        &mut self,
        r: &Renderer,
        frame: &mut Frame,
        kamera: &Kamera3d,
        modelle: &[Modell],
        bereich: Bereich,
    ) {
        let flaeche = r.groesse();
        let x = bereich.x.clamp(0.0, flaeche.x);
        let y = bereich.y.clamp(0.0, flaeche.y);
        let b = bereich.breite.min(flaeche.x - x);
        let h = bereich.hoehe.min(flaeche.y - y);
        if b < 1.0 || h < 1.0 {
            return;
        }
        let device = r.device();
        // Tiefenpuffer in Fenstergröße (neu bei Größenänderung)
        let (fb, fh) = (flaeche.x as u32, flaeche.y as u32);
        if self.tiefe.as_ref().is_none_or(|(_, w, hh)| *w != fb || *hh != fh) {
            let t = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Tiefe"),
                size: wgpu::Extent3d {
                    width: fb,
                    height: fh,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TIEFENFORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.tiefe = Some((t.create_view(&Default::default()), fb, fh));
        }

        let licht = Vec3::new(0.45, 0.85, 0.3).normalize();
        let pos = kamera.position();
        let daten = SzenenDaten {
            ansicht_projektion: kamera.matrix(b / h).to_cols_array_2d(),
            kamera: [pos.x, pos.y, pos.z, 0.0],
            licht: [licht.x, licht.y, licht.z, 0.0],
            nebel: [0.0044, 0.0109, 0.0232, 0.012],
        };
        r.queue()
            .write_buffer(&self.szenen_puffer, 0, bytemuck::bytes_of(&daten));

        self.sortiert.clear();
        self.sortiert.extend(modelle.iter().map(|m| {
            (
                (m.unbeleuchtet, m.form),
                Instanz {
                    matrix: m.matrix.to_cols_array_2d(),
                    farbe: m.farbe,
                    extra: [if m.unbeleuchtet { 1.0 } else { 0.0 }, 0.0, 0.0, 0.0],
                },
            )
        }));
        self.sortiert.sort_by_key(|(f, _)| *f);
        if self.sortiert.len() > self.instanz_kapazitaet {
            self.instanz_kapazitaet = self.sortiert.len().next_power_of_two();
            self.instanz_puffer = instanz_puffer(device, self.instanz_kapazitaet);
        }
        let instanzen: Vec<Instanz> = self.sortiert.iter().map(|(_, i)| *i).collect();
        if !instanzen.is_empty() {
            r.queue()
                .write_buffer(&self.instanz_puffer, 0, bytemuck::cast_slice(&instanzen));
        }

        let tiefe = &self.tiefe.as_ref().unwrap().0;
        let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("3D"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &frame.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: tiefe,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_viewport(x, y, b, h, 0.0, 1.0);
        pass.set_scissor_rect(x as u32, y as u32, b as u32, h as u32);
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.szenen_gruppe, &[]);
        pass.set_vertex_buffer(1, self.instanz_puffer.slice(..));
        let mut start = 0;
        while start < self.sortiert.len() {
            let gruppe = self.sortiert[start].0;
            let form = gruppe.1;
            let mut ende = start;
            while ende < self.sortiert.len() && self.sortiert[ende].0 == gruppe {
                ende += 1;
            }
            let (_, mesh) = self.meshes.iter().find(|(f, _)| *f == form).unwrap();
            pass.set_vertex_buffer(0, mesh.ecken.slice(..));
            pass.set_index_buffer(mesh.indizes.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.anzahl, 0, start as u32..ende as u32);
            start = ende;
        }
    }
}

fn instanz_puffer(device: &wgpu::Device, kapazitaet: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Mesh-Instanzen"),
        size: (kapazitaet * std::mem::size_of::<Instanz>()) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Ecken und Dreiecke eines Körpers (Größe 1, Mitte im Ursprung).
fn koerper(form: Form3d) -> (Vec<Ecke>, Vec<u32>) {
    let mut ecken = Vec::new();
    let mut indizes = Vec::new();
    let flaeche = |ecken: &mut Vec<Ecke>, indizes: &mut Vec<u32>, punkte: [Vec3; 4], n: Vec3| {
        let basis = ecken.len() as u32;
        for p in punkte {
            ecken.push(Ecke {
                position: p.to_array(),
                normale: n.to_array(),
            });
        }
        indizes.extend([basis, basis + 1, basis + 2, basis, basis + 2, basis + 3]);
    };
    match form {
        Form3d::Wuerfel => {
            for (n, u, v) in [
                (Vec3::X, Vec3::Y, Vec3::Z),
                (Vec3::NEG_X, Vec3::Z, Vec3::Y),
                (Vec3::Y, Vec3::Z, Vec3::X),
                (Vec3::NEG_Y, Vec3::X, Vec3::Z),
                (Vec3::Z, Vec3::X, Vec3::Y),
                (Vec3::NEG_Z, Vec3::Y, Vec3::X),
            ] {
                let m = n * 0.5;
                flaeche(
                    &mut ecken,
                    &mut indizes,
                    [
                        m - u * 0.5 - v * 0.5,
                        m + u * 0.5 - v * 0.5,
                        m + u * 0.5 + v * 0.5,
                        m - u * 0.5 + v * 0.5,
                    ],
                    n,
                );
            }
        }
        Form3d::Ebene => {
            flaeche(
                &mut ecken,
                &mut indizes,
                [
                    Vec3::new(-0.5, 0.0, -0.5),
                    Vec3::new(-0.5, 0.0, 0.5),
                    Vec3::new(0.5, 0.0, 0.5),
                    Vec3::new(0.5, 0.0, -0.5),
                ],
                Vec3::Y,
            );
        }
        Form3d::Kugel => {
            let (ringe, segmente) = (16u32, 28u32);
            for r in 0..=ringe {
                let theta = r as f32 / ringe as f32 * std::f32::consts::PI;
                for s in 0..=segmente {
                    let phi = s as f32 / segmente as f32 * std::f32::consts::TAU;
                    let n = Vec3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin());
                    ecken.push(Ecke {
                        position: (n * 0.5).to_array(),
                        normale: n.to_array(),
                    });
                }
            }
            for r in 0..ringe {
                for s in 0..segmente {
                    let a = r * (segmente + 1) + s;
                    let b = a + segmente + 1;
                    indizes.extend([a, b, a + 1, a + 1, b, b + 1]);
                }
            }
        }
        Form3d::Zylinder => {
            let segmente = 28u32;
            for s in 0..=segmente {
                let phi = s as f32 / segmente as f32 * std::f32::consts::TAU;
                let n = Vec3::new(phi.cos(), 0.0, phi.sin());
                for y in [-0.5, 0.5] {
                    ecken.push(Ecke {
                        position: (n * 0.5 + Vec3::Y * y).to_array(),
                        normale: n.to_array(),
                    });
                }
            }
            for s in 0..segmente {
                let a = s * 2;
                indizes.extend([a, a + 1, a + 2, a + 2, a + 1, a + 3]);
            }
            // Deckel
            for (y, n) in [(0.5f32, Vec3::Y), (-0.5, Vec3::NEG_Y)] {
                let mitte = ecken.len() as u32;
                ecken.push(Ecke {
                    position: [0.0, y, 0.0],
                    normale: n.to_array(),
                });
                for s in 0..=segmente {
                    let phi = s as f32 / segmente as f32 * std::f32::consts::TAU;
                    ecken.push(Ecke {
                        position: [phi.cos() * 0.5, y, phi.sin() * 0.5],
                        normale: n.to_array(),
                    });
                }
                for s in 0..segmente {
                    indizes.extend([mitte, mitte + 1 + s, mitte + 2 + s]);
                }
            }
        }
    }
    (ecken, indizes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn koerper_sind_vollstaendig() {
        for f in Form3d::ALLE {
            let (ecken, indizes) = koerper(f);
            assert!(!ecken.is_empty() && indizes.len() % 3 == 0, "{f:?}");
            assert!(indizes.iter().all(|i| (*i as usize) < ecken.len()), "{f:?}");
            assert_eq!(Form3d::aus_schluessel(f.schluessel()), Some(f));
        }
    }

    #[test]
    fn strahl_trifft_die_mitte() {
        let k = Kamera3d::default();
        let (start, richtung) = k.strahl(glam::vec2(400.0, 300.0), glam::vec2(800.0, 600.0));
        // Der Strahl durch die Bildmitte geht durchs Ziel
        let naechster = start + richtung * (k.ziel - start).dot(richtung);
        assert!(naechster.distance(k.ziel) < 1e-3, "{naechster}");
    }
}
