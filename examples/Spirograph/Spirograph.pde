// Spirograph – layered rotating arms drawing a hypotrochoid curve.
// Demonstrates: push_matrix / pop_matrix, translate, rotate, line,
//               ellipse, no_fill, stroke_rgba, frame_count, TWO_PI.

let r1: f32 = 140.0;  // outer arm radius
let r2: f32 = 60.0;   // inner arm radius
let pen: f32 = 90.0;  // pen offset on inner arm

fn setup() {
    size(600, 600);
    frameRate(60.0);
    background(10.0);
}

fn draw() {
    // Fade the trail slowly so the pattern accumulates.
    fill_rgba(10.0, 10.0, 10.0, 12.0);
    no_stroke();
    rect(0.0, 0.0, width(), height());

    let t = frame_count() as f32 * 0.025;

    push_matrix();
    translate(width() / 2.0, height() / 2.0);

    // Outer arm.
    rotate(t);
    let x1 = r1;
    let y1 = 0.0;

    // Inner arm (relative to outer arm tip).
    let t2 = -t * (r1 / r2);
    let x2 = x1 + r2 * t2.cos();
    let y2 = y1 + r2 * t2.sin();

    // Pen position.
    let px = x1 + pen * t2.cos();
    let py = y1 + pen * t2.sin();

    // Draw the dot at the pen.
    no_stroke();
    let hue = (t * 0.3) % 1.0;
    // HSV→RGB: full-sat hue cycle.
    let (dr, dg, db) = hsv_to_rgb(hue, 1.0, 1.0);
    fill_rgba(dr, dg, db, 200.0);
    circle(px, py, 5.0);

    // Draw arm structure faintly.
    stroke_rgba(255.0, 255.0, 255.0, 30.0);
    stroke_weight(1.0);
    no_fill();
    ellipse(0.0, 0.0, r1 * 2.0, r1 * 2.0);
    ellipse(x1, y1, r2 * 2.0, r2 * 2.0);

    pop_matrix();
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let i = (h * 6.0).floor() as i32 % 6;
    let f = h * 6.0 - (h * 6.0).floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    (r * 255.0, g * 255.0, b * 255.0)
}
