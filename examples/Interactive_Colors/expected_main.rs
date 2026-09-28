// Expected emitter output for Interactive_Colors.pde
use processing::*;

// ── SketchState ──────────────────────────────────────────────────────────────
struct SketchState {
    bg: f32,
    r: f32,
    g: f32,
    b: f32,
}

impl SketchState {
    fn new() -> Self {
        let mut bg: f32 = 0.0;
        let mut r: f32 = 200.0;
        let mut g: f32 = 100.0;
        let mut b: f32 = 50.0;
        Self { bg, r, g, b }
    }
}

// ── sketch lifecycle ─────────────────────────────────────────────────────────
fn setup(s: &mut SketchState) {
    size(600, 400);
    textSize(18.0);
}

fn draw(s: &mut SketchState) {
    background(s.bg);
    fill_rgb(s.r, s.g, s.b);
    ellipse(mouseX(), mouseY(), 60.0, 60.0);

    fill(220.0);
    text("Click to randomise   |   any key to reset", 20.0, 30.0);
}

fn mouse_pressed(s: &mut SketchState) {
    s.bg = random(255.0);
    s.r  = random(255.0);
    s.g  = random(255.0);
    s.b  = random(255.0);
}

fn key_pressed(s: &mut SketchState) {
    s.bg = 0.0;
    s.r  = 200.0;
    s.g  = 100.0;
    s.b  = 50.0;
}

// ── entry point ──────────────────────────────────────────────────────────────
fn main() {
    processing::App::new(SketchState::new(), setup, draw)
        .title("Interactive_Colors")
        .mouse_pressed(mouse_pressed)
        .key_pressed(key_pressed)
        .run();
}
