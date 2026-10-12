//! Timbre-Starter: öffnet ein Fenster (nativ) bzw. ein Canvas (Browser),
//! treibt den Game Loop an und lässt den Renderer zeichnen.

mod spiel;
mod texturen;

use std::sync::Arc;

use spiel::{Anzeige, Spiel};
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
    spiel: Option<Spiel>,
    input: Input,
    takt: FesterTakt,
    anzeige: Anzeige,
    minimiert: bool,
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<TimbreEvent>>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            renderer: None,
            spiel: None,
            input: Input::default(),
            takt: FesterTakt::new(LOGIK_HZ),
            anzeige: Anzeige::default(),
            minimiert: false,
            #[cfg(target_arch = "wasm32")]
            proxy: None,
        }
    }

    /// Renderer ist fertig (nativ sofort, im Browser später): Spiel aufbauen und loslegen.
    fn renderer_bereit(&mut self, mut renderer: Renderer) {
        if let Some(w) = self.window.as_ref() {
            // Größe kann sich geändert haben, während der Renderer gebaut wurde.
            let size = w.inner_size();
            renderer.resize(size.width, size.height);
            w.request_redraw();
        }
        self.spiel = Some(Spiel::new(&mut renderer));
        self.renderer = Some(renderer);
    }

    /// Ein gezeichneter Frame: fällige Logik-Schritte rechnen, dann zeichnen.
    fn frame(&mut self) {
        let (Some(window), Some(renderer), Some(spiel)) = (
            self.window.as_ref(),
            self.renderer.as_mut(),
            self.spiel.as_mut(),
        ) else {
            return;
        };
        if self.minimiert {
            return; // nicht zeichnen; nach dem Wiederherstellen geht es weiter
        }

        let schritte = self.takt.frame();
        let bildschirm = renderer.groesse();
        for _ in 0..schritte {
            spiel.schritt(self.takt.dt(), &self.input, bildschirm);
            self.input.schritt_ende();
        }

        let alpha = self.takt.alpha();
        spiel.sprites_bauen(alpha);
        let stat = renderer.zeichnen(&spiel.kamera(alpha), spiel.sprites());
        self.anzeige.frame(schritte, stat, window);

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
        let attrs = Window::default_attributes().with_title(format!(
            "{} {}",
            timbre_core::NAME,
            timbre_core::VERSION
        ));

        // Nativ: feste Startgröße. Browser: Canvas an die Seite hängen, Größe kommt aus dem CSS.
        #[cfg(not(target_arch = "wasm32"))]
        let attrs = attrs.with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
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
        self.input.verarbeite(&event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            #[cfg(not(target_arch = "wasm32"))]
            WindowEvent::KeyboardInput {
                event: ref taste, ..
            } if taste.physical_key
                == winit::keyboard::PhysicalKey::Code(timbre_core::KeyCode::Escape) =>
            {
                event_loop.exit()
            }
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
        event_loop
            .run_app(&mut app)
            .expect("Event-Loop abgebrochen");
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
        env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn"),
    )
    .init();
}

#[cfg(target_arch = "wasm32")]
fn logging_starten() {
    // Panics und Logs landen in der Browser-Konsole (F12).
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("Logging konnte nicht gestartet werden");
}
