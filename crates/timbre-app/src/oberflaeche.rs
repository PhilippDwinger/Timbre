//! Verbindet egui (die Editor-Oberfläche) mit Fenster (winit) und Grafikkarte (wgpu).

use timbre_core::{Frame, Renderer};
use winit::window::Window;

pub struct Oberflaeche {
    pub ctx: egui::Context,
    zustand: egui_winit::State,
    renderer: egui_wgpu::Renderer,
}

impl Oberflaeche {
    pub fn new(window: &Window, renderer: &Renderer) -> Self {
        let ctx = egui::Context::default();
        ctx.set_visuals(egui::Visuals::dark());
        let zustand = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            Some(renderer.device().limits().max_texture_dimension_2d as usize),
        );
        let renderer = egui_wgpu::Renderer::new(
            renderer.device(),
            renderer.format_linear(),
            egui_wgpu::RendererOptions::default(),
        );
        Self { ctx, zustand, renderer }
    }

    /// Fenster-Ereignis an egui geben. `true` = egui hat es verbraucht.
    pub fn ereignis(&mut self, window: &Window, event: &winit::event::WindowEvent) -> bool {
        self.zustand.on_window_event(window, event).consumed
    }

    /// Oberfläche für diesen Frame aufbauen.
    pub fn ausfuehren(&mut self, window: &Window, ui: impl FnMut(&mut egui::Ui)) -> egui::FullOutput {
        let eingabe = self.zustand.take_egui_input(window);
        let mut ausgabe = self.ctx.run_ui(eingabe, ui);
        self.zustand
            .handle_platform_output(window, std::mem::take(&mut ausgabe.platform_output));
        ausgabe
    }

    /// Neue/geänderte Schrift- und Bildtexturen hochladen. Muss jeden Frame passieren –
    /// auch wenn gerade nicht gezeichnet werden kann.
    pub fn texturen_aktualisieren(&mut self, renderer: &Renderer, ausgabe: &mut egui::FullOutput) {
        for (id, deltas) in &ausgabe.textures_delta.set {
            for delta in deltas {
                self.renderer
                    .update_texture(renderer.device(), renderer.queue(), *id, delta);
            }
        }
        for id in &ausgabe.textures_delta.free {
            self.renderer.free_texture(id);
        }
        ausgabe.textures_delta.clear();
    }

    /// Oberfläche über die Szene zeichnen.
    pub fn zeichnen(&mut self, renderer: &Renderer, frame: &mut Frame, ausgabe: egui::FullOutput, pixel: [u32; 2]) {
        let ppp = ausgabe.pixels_per_point;
        let formen = self.ctx.tessellate(ausgabe.shapes, ppp);
        let bildschirm = egui_wgpu::ScreenDescriptor {
            size_in_pixels: pixel,
            pixels_per_point: ppp,
        };
        self.renderer.update_buffers(
            renderer.device(),
            renderer.queue(),
            &mut frame.encoder,
            &formen,
            &bildschirm,
        );
        let mut pass = frame
            .encoder
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Oberfläche"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.view_linear,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            })
            .forget_lifetime();
        self.renderer.render(&mut pass, &formen, &bildschirm);
    }
}
