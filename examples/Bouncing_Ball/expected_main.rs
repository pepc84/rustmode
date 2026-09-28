// Expected output from RustEmitter for Bouncing_Ball.pde
// Used to manually validate the transform pipeline.
use processing::*;

// ── SketchState ──────────────────────────────────────────────────────────────
struct SketchState {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    radius: f32,
}

impl SketchState {
    fn new() -> Self {
        let mut x: f32 = 300.0;
        let mut y: f32 = 300.0;
        let mut vx: f32 = 3.0;
        let mut vy: f32 = 2.5;
        let radius: f32 = 30.0;
        Self { x, y, vx, vy, radius }
    }
}

// ── sketch lifecycle ─────────────────────────────────────────────────────────
fn setup(s: &mut SketchState) {
    size(600, 600);
}

fn draw(s: &mut SketchState) {
    background(30.0);

    s.x += s.vx;
    s.y += s.vy;

    if s.x - s.radius < 0.0 || s.x + s.radius > width() {
        s.vx = -s.vx;
    }
    if s.y - s.radius < 0.0 || s.y + s.radius > height() {
        s.vy = -s.vy;
    }

    fill(180.0);
    ellipse(s.x, s.y, s.radius * 2.0, s.radius * 2.0);
}

// ── entry point ──────────────────────────────────────────────────────────────
fn main() {
    processing::App::new(SketchState::new(), setup, draw)
        .title("Bouncing_Ball")
        .run();
}
