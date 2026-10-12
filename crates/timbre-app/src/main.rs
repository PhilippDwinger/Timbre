//! Timbre Studio – startet Fenster (nativ) bzw. Canvas (Browser), treibt den Game Loop an
//! und zeichnet Szene und Editor-Oberfläche.

mod luau_farben;
mod oberflaeche;
mod projekt;
mod studio;

use std::sync::Arc;

use oberflaeche::Oberflaeche;
use studio::Studio;
use timbre_core::{FesterTakt, Input, Renderer};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// Logik-Schritte pro Sekunde.
const LOGIK_HZ: u32 = 60;

/// Eigene Ereignisse der Engine.
/// Im Browser wird der Renderer asynchron gebaut und kommt über dieses Ereignis zurück.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // nativ ungenutzt
enum TimbreEvent {
    RendererBereit(Renderer),
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    oberflaeche: Option<Oberflaeche>,
    studio: Option<Studio>,
    input: Input,
    takt: FesterTakt,
    minimiert: bool,
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<TimbreEvent>>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            renderer: None,
            oberflaeche: None,
            studio: None,
            input: Input::default(),
            takt: FesterTakt::new(LOGIK_HZ),
            minimiert: false,
            #[cfg(target_arch = "wasm32")]
            proxy: None,
        }
    }

    /// Renderer ist fertig (nativ sofort, im Browser später): Studio aufbauen und loslegen.
    fn renderer_bereit(&mut self, mut renderer: Renderer) {
        let Some(w) = self.window.clone() else { return };
        // Größe kann sich geändert haben, während der Renderer gebaut wurde.
        let size = w.inner_size();
        renderer.resize(size.width, size.height);
        let studio = Studio::new(&mut renderer);
        w.set_title(&studio.titel());
        self.oberflaeche = Some(Oberflaeche::new(&w, &renderer));
        self.studio = Some(studio);
        self.renderer = Some(renderer);
        w.request_redraw();
    }

    /// Ein gezeichneter Frame: Oberfläche, fällige Logik-Schritte, dann zeichnen.
    fn frame(&mut self) {
        let App {
            window,
            renderer,
            oberflaeche,
            studio,
            input,
            takt,
            minimiert,
            ..
        } = self;
        let (Some(window), Some(renderer), Some(ober), Some(studio)) = (
            window.as_ref(),
            renderer.as_mut(),
            oberflaeche.as_mut(),
            studio.as_mut(),
        ) else {
            return;
        };
        if *minimiert {
            return; // nicht zeichnen; nach dem Wiederherstellen geht es weiter
        }

        // 1. Oberfläche (Knöpfe, Klicks in die Szene, Script-Editor …)
        let mut ausgabe = ober.ausfuehren(window, |ui| studio.ui(ui));
        ober.texturen_aktualisieren(renderer, &mut ausgabe);
        let tastatur_frei = !ober.ctx.egui_wants_keyboard_input();

        // 2. Logik im festen Takt
        let schritte = takt.frame();
        for _ in 0..schritte {
            studio.schritt(takt.dt(), input, tastatur_frei);
            input.schritt_ende();
        }

        // 3. Zeichnen: Szene in ihren Bereich, Oberfläche darüber
        if let Some(mut frame) = renderer.frame_beginnen() {
            let mut statistik = Default::default();
            if let Some(bereich) = studio.bereich() {
                let (kamera, sprites) = studio.sprites_bauen(takt.alpha());
                statistik = renderer.sprites_zeichnen(&mut frame, &kamera, sprites, bereich);
            }
            studio.statistik_setzen(statistik);
            let size = window.inner_size();
            ober.zeichnen(renderer, &mut frame, ausgabe, [size.width, size.height]);
            renderer.frame_beenden(frame);
        }

        // Sofort den nächsten Frame anfordern → Dauerschleife im Takt des Monitors (VSync).
        window.request_redraw();
    }
}

impl ApplicationHandler<TimbreEvent> for App {
    /// Wird aufgerufen, sobald das System bereit ist – hier entsteht das Fenster.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("Timbre Studio");

        // Nativ: feste Startgröße. Browser: Canvas an die Seite hängen, Größe kommt aus dem CSS.
        #[cfg(not(target_arch = "wasm32"))]
        let attrs = attrs.with_inner_size(winit::dpi::LogicalSize::new(1440.0, 860.0));
        #[cfg(target_arch = "wasm32")]
        let attrs = {
            use winit::platform::web::WindowAttributesExtWebSys;
            attrs.with_append(true)
        };

        let window = Arc::new(
            event_loop
                .create_window(attrs)
                .expect("Fenster konnte nicht erstellt werden"),
        );
        let size = window.inner_size();
        self.window = Some(window.clone());

        // Nativ dürfen wir auf die Grafikkarte warten.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let renderer = pollster::block_on(Renderer::new(window, size.width, size.height));
            self.renderer_bereit(renderer);
        }

        // Im Browser darf nichts blockieren: Renderer im Hintergrund bauen, danach Ereignis schicken.
        #[cfg(target_arch = "wasm32")]
        {
            let proxy = self.proxy.take().expect("Proxy fehlt");
            wasm_bindgen_futures::spawn_local(async move {
                let renderer = Renderer::new(window, size.width, size.height).await;
                let _ = proxy.send_event(TimbreEvent::RendererBereit(renderer));
            });
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: TimbreEvent) {
        match event {
            TimbreEvent::RendererBereit(renderer) => self.renderer_bereit(renderer),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let (Some(w), Some(o)) = (self.window.as_ref(), self.oberflaeche.as_mut()) {
            o.ereignis(w, &event);
        }
        self.input.verarbeite(&event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                self.minimiert = size.width == 0 || size.height == 0;
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }
}

fn main() {
    logging_starten();
    log::info!("{}", timbre_core::banner());

    let event_loop = EventLoop::<TimbreEvent>::with_user_event()
        .build()
        .expect("Event-Loop konnte nicht erstellt werden");
    // Wait: Die Schleife schläft, bis ein Ereignis kommt. Weil jeder Frame den nächsten
    // anfordert, läuft sie trotzdem durchgehend – aber im Takt des Monitors statt mit 100 % CPU.
    event_loop.set_control_flow(ControlFlow::Wait);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = App::new();
        event_loop.run_app(&mut app).expect("Event-Loop abgebrochen");
    }

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        let mut app = App::new();
        app.proxy = Some(event_loop.create_proxy());
        event_loop.spawn_app(app);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn logging_starten() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn,naga=warn"),
    )
    .init();
}

#[cfg(target_arch = "wasm32")]
fn logging_starten() {
    // Panics und Logs landen in der Browser-Konsole (F12).
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("Logging konnte nicht gestartet werden");
}
