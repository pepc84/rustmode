// Bouncing Ball – minimal RustMode sketch
// Demonstrates: size, background, fill, ellipse, sketch variables

let mut x: f32 = 300.0;
let mut y: f32 = 300.0;
let mut vx: f32 = 3.0;
let mut vy: f32 = 2.5;
let radius: f32 = 30.0;

fn setup() {
    size(600, 600);
}

fn draw() {
    background(30.0);

    x += vx;
    y += vy;

    if x - radius < 0.0 || x + radius > width() {
        vx = -vx;
    }
    if y - radius < 0.0 || y + radius > height() {
        vy = -vy;
    }

    fill(180.0);
    ellipse(x, y, radius * 2.0, radius * 2.0);
}
