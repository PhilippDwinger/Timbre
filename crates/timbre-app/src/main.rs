//! Timbre-Starter: öffnet ein Fenster (nativ) bzw. ein Canvas (Browser)
//! und lässt den Renderer darin zeichnen.

use std::sync::Arc;

use timbre_core::Renderer;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// Eigene Ereignisse der Engine.
/// Im Browser wird der Renderer asynchron gebaut und kommt über dieses Ereignis zurück.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // nativ ungenutzt
enum TimbreEvent {
    RendererBereit(Renderer),
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    #[cfg(target_arch = "wasm32")]
    proxy: Option<winit::event_loop::EventLoopProxy<TimbreEvent>>,
}

impl ApplicationHandler<TimbreEvent> for App {
    /// Wird aufgerufen, sobald das System bereit ist – hier entsteht das Fenster.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(format!("{} {}", timbre_core::NAME, timbre_core::VERSION));

        // Nativ: feste Startgröße. Browser: Canvas an die Seite hängen, Größe kommt aus dem CSS.
        #[cfg(not(target_arch = "wasm32"))]
        let attrs = attrs.with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        #[cfg(target_arch = "wasm32")]
        let attrs = {
            use winit::platform::web::WindowAttributesExtWebSys;
            attrs.with_append(true)
        };

        let window = Arc::new(event_loop.create_window(attrs).expect("Fenster konnte nicht erstellt werden"));
        let size = window.inner_size();

        // Nativ dürfen wir auf die Grafikkarte warten.
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.renderer = Some(pollster::block_on(Renderer::new(window.clone(), size.width, size.height)));
            window.request_redraw();
        }

        // Im Browser darf nichts blockieren: Renderer im Hintergrund bauen, danach Ereignis schicken.
        #[cfg(target_arch = "wasm32")]
        {
            let proxy = self.proxy.take().expect("Proxy fehlt");
            let w = window.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let renderer = Renderer::new(w, size.width, size.height).await;
                let _ = proxy.send_event(TimbreEvent::RendererBereit(renderer));
            });
        }

        self.window = Some(window);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: TimbreEvent) {
        match event {
            TimbreEvent::RendererBereit(mut renderer) => {
                if let Some(w) = self.window.as_ref() {
                    // Größe kann sich geändert haben, während der Renderer gebaut wurde.
                    let size = w.inner_size();
                    renderer.resize(size.width, size.height);
                    w.request_redraw();
                }
                self.renderer = Some(renderer);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(r) = self.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(r) = self.renderer.as_mut() {
                    r.render();
                }
            }
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
    // Wait: nur neu zeichnen, wenn nötig (spart Strom). Ab Meilenstein 2 kommt der Game Loop.
    event_loop.set_control_flow(ControlFlow::Wait);

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut app = App { window: None, renderer: None };
        event_loop.run_app(&mut app).expect("Event-Loop abgebrochen");
    }

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::EventLoopExtWebSys;
        let app = App { window: None, renderer: None, proxy: Some(event_loop.create_proxy()) };
        event_loop.spawn_app(app);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn logging_starten() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu_core=warn,wgpu_hal=warn"))
        .init();
}

#[cfg(target_arch = "wasm32")]
fn logging_starten() {
    // Panics und Logs landen in der Browser-Konsole (F12).
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("Logging konnte nicht gestartet werden");
}
