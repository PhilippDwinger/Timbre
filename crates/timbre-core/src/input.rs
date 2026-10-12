//! Eingabe: Tastatur und Maus.
//!
//! Merkt sich, was gerade gedrückt ist (`taste`) und was seit dem letzten
//! Logik-Schritt neu gedrückt wurde (`taste_neu`). Tasten werden über ihre
//! Position auf der Tastatur erkannt (`KeyCode::KeyW` ist auf QWERTZ und AZERTY
//! dieselbe Stelle) – praktisch für WASD.

use std::collections::HashSet;

use glam::Vec2;
pub use winit::event::MouseButton;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
pub use winit::keyboard::KeyCode;
use winit::keyboard::PhysicalKey;

#[derive(Clone, Default)]
pub struct Input {
    gehalten: HashSet<KeyCode>,
    neu: HashSet<KeyCode>,
    maus_gehalten: HashSet<MouseButton>,
    maus_neu: HashSet<MouseButton>,
    maus_pos: Vec2,
    scroll: f32,
}

impl Input {
    /// Jedes Fenster-Ereignis hier durchreichen.
    pub fn verarbeite(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            if !event.repeat {
                                self.neu.insert(code);
                            }
                            self.gehalten.insert(code);
                        }
                        ElementState::Released => {
                            self.gehalten.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => {
                    self.maus_neu.insert(*button);
                    self.maus_gehalten.insert(*button);
                }
                ElementState::Released => {
                    self.maus_gehalten.remove(button);
                }
            },
            WindowEvent::CursorMoved { position, .. } => {
                self.maus_pos = Vec2::new(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.scroll += match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 50.0,
                };
            }
            // Fenster verliert den Fokus: sonst "klemmen" Tasten, die woanders losgelassen werden.
            WindowEvent::Focused(false) => {
                self.gehalten.clear();
                self.maus_gehalten.clear();
            }
            _ => {}
        }
    }

    /// Taste ist gerade gedrückt.
    pub fn taste(&self, k: KeyCode) -> bool {
        self.gehalten.contains(&k)
    }

    /// Taste wurde seit dem letzten Logik-Schritt neu gedrückt.
    pub fn taste_neu(&self, k: KeyCode) -> bool {
        self.neu.contains(&k)
    }

    pub fn maustaste(&self, b: MouseButton) -> bool {
        self.maus_gehalten.contains(&b)
    }

    pub fn maustaste_neu(&self, b: MouseButton) -> bool {
        self.maus_neu.contains(&b)
    }

    /// Mausposition in Pixeln, (0,0) = links oben.
    pub fn maus_position(&self) -> Vec2 {
        self.maus_pos
    }

    /// Mausrad seit dem letzten Logik-Schritt (positiv = nach oben).
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// Nach jedem Logik-Schritt aufrufen: setzt "neu gedrückt" und Mausrad zurück.
    pub fn schritt_ende(&mut self) {
        self.neu.clear();
        self.maus_neu.clear();
        self.scroll = 0.0;
    }
}

/// Tastenname aus einem Script → Taste. Groß-/Kleinschreibung egal.
/// Buchstaben ("W"), Ziffern ("1"), "Space", "Enter", "Escape", "Shift", "Ctrl", "Tab",
/// "Up", "Down", "Left", "Right".
pub fn taste_aus_name(name: &str) -> Option<KeyCode> {
    use KeyCode::*;
    let n = name.trim().to_ascii_lowercase();
    if n.len() == 1 {
        let c = n.chars().next()?;
        const BUCHSTABEN: [KeyCode; 26] = [
            KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN,
            KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
        ];
        const ZIFFERN: [KeyCode; 10] = [
            Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
        ];
        if c.is_ascii_lowercase() {
            return Some(BUCHSTABEN[(c as u8 - b'a') as usize]);
        }
        if c.is_ascii_digit() {
            return Some(ZIFFERN[(c as u8 - b'0') as usize]);
        }
    }
    Some(match n.as_str() {
        "space" | "leertaste" => Space,
        "enter" | "return" => Enter,
        "escape" | "esc" => Escape,
        "shift" => ShiftLeft,
        "ctrl" | "strg" => ControlLeft,
        "tab" => Tab,
        "up" | "hoch" => ArrowUp,
        "down" | "runter" => ArrowDown,
        "left" | "links" => ArrowLeft,
        "right" | "rechts" => ArrowRight,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tastennamen() {
        assert_eq!(taste_aus_name("W"), Some(KeyCode::KeyW));
        assert_eq!(taste_aus_name("space"), Some(KeyCode::Space));
        assert_eq!(taste_aus_name("7"), Some(KeyCode::Digit7));
        assert_eq!(taste_aus_name("quatsch"), None);
    }
}
