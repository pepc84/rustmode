// Clock – analog clock face
// Demonstrates: millis(), sin/cos, push_matrix/pop_matrix, stroke_weight, no_fill

fn setup() {
    size(400, 400);
}

fn draw() {
    background(20.0);

    let cx: f32 = width() / 2.0;
    let cy: f32 = height() / 2.0;

    // Clock face
    stroke(200.0);
    stroke_weight(2.0);
    no_fill();
    ellipse(cx, cy, 360.0, 360.0);

    // Hour markers
    let mut i: i32 = 0;
    while i < 12 {
        let angle: f32 = (i as f32) * TWO_PI / 12.0 - HALF_PI;
        let x1: f32 = cx + 160.0 * angle.cos();
        let y1: f32 = cy + 160.0 * angle.sin();
        let x2: f32 = cx + 175.0 * angle.cos();
        let y2: f32 = cy + 175.0 * angle.sin();
        line(x1, y1, x2, y2);
        i += 1;
    }

    // Derive H/M/S from millis()
    let total_secs: f32 = millis() as f32 / 1000.0;
    let secs:  f32 = total_secs % 60.0;
    let mins:  f32 = (total_secs / 60.0) % 60.0;
    let hours: f32 = (total_secs / 3600.0) % 12.0;

    // Second hand
    let sa: f32 = secs * TWO_PI / 60.0 - HALF_PI;
    stroke(220.0);
    stroke_weight(1.5);
    line(cx, cy, cx + 155.0 * sa.cos(), cy + 155.0 * sa.sin());

    // Minute hand
    let ma: f32 = mins * TWO_PI / 60.0 - HALF_PI;
    stroke(200.0);
    stroke_weight(3.0);
    line(cx, cy, cx + 130.0 * ma.cos(), cy + 130.0 * ma.sin());

    // Hour hand
    let ha: f32 = hours * TWO_PI / 12.0 - HALF_PI;
    stroke(200.0);
    stroke_weight(5.0);
    line(cx, cy, cx + 90.0 * ha.cos(), cy + 90.0 * ha.sin());

    // Centre dot
    fill(255.0);
    no_stroke();
    circle(cx, cy, 8.0);
}
