// NoiseTerrain – scrolling landscape generated with Perlin noise.
// Move the mouse left/right to control scroll speed.
// Demonstrates: noise(), map(), line, stroke_weight, stroke_rgb, push_matrix / pop_matrix.

let mut offset: f32 = 0.0;

fn setup() {
    size(800, 400);
    frameRate(60.0);
}

fn draw() {
    background(18.0);

    // Scroll speed driven by mouse X.
    let speed = map(mouseX(), 0.0, width(), -0.015, 0.015);
    offset += speed;

    // Draw sky-gradient columns as thin vertical lines.
    let cols: i32 = width() as i32;
    let step = 1.0;
    let mut x: f32 = 0.0;
    while x < width() {
        let nx = x / width() + offset;
        let h = noise(nx) * height() * 0.7;
        let ground_y = height() - h;

        // Sky column.
        stroke_rgb(30.0, 60.0 + h * 0.15, 100.0 + h * 0.2);
        stroke_weight(step);
        line(x, 0.0, x, ground_y);

        // Ground column: colour based on height.
        let g = map(h, 0.0, height() * 0.7, 40.0, 160.0);
        stroke_rgb(30.0, g, 30.0);
        line(x, ground_y, x, height());

        x += step;
    }

    // HUD.
    stroke_weight(1.0);
    no_stroke();
    fill(220.0);
    text_size(13.0);
    text("Mouse X: scroll speed", 10.0, 16.0);
}
