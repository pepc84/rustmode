// Particles – simple particle system
// Click to burst; hold mouse to stream.
// Demonstrates: Vec, for loops, mousePressed, text_width, rect_rounded.

let mut px: Vec<f32> = Vec::new();
let mut py: Vec<f32> = Vec::new();
let mut pvx: Vec<f32> = Vec::new();
let mut pvy: Vec<f32> = Vec::new();
let mut plife: Vec<f32> = Vec::new();

fn setup() {
    size(700, 500);
    frameRate(60.0);
}

fn draw() {
    background(15.0);

    // Spawn when mouse is held.
    if mousePressed() {
        let mx = mouseX();
        let my = mouseY();
        for _ in 0..5 {
            px.push(mx);
            py.push(my);
            pvx.push(random_range(-3.0, 3.0));
            pvy.push(random_range(-5.0, -1.0));
            plife.push(1.0);
        }
    }

    // Update & draw particles.
    let mut i = 0;
    while i < px.len() {
        plife[i] -= 0.02;
        if plife[i] <= 0.0 {
            px.remove(i);
            py.remove(i);
            pvx.remove(i);
            pvy.remove(i);
            plife.remove(i);
            continue;
        }
        pvx[i] *= 0.98;
        pvy[i] += 0.15; // gravity
        px[i] += pvx[i];
        py[i] += pvy[i];

        let alpha = plife[i];
        fill_rgba(255.0, 140.0, 30.0, alpha * 255.0);
        no_stroke();
        let r = 6.0 * alpha;
        circle(px[i], py[i], r * 2.0);
        i += 1;
    }

    // HUD pill
    let label = "Particles: ";
    let count_str = format!("{}", px.len());
    let full = format!("{}{}", label, count_str);
    let tw = text_width(&full);
    fill_rgba(0.0, 0.0, 0.0, 160.0);
    no_stroke();
    rect_rounded(14.0, 14.0, tw + 20.0, 32.0, 8.0);
    fill(220.0);
    text_size(14.0);
    text(&full, 24.0, 20.0);
}

fn mouse_pressed() {
    // Burst on click: spawn 30 at once.
    let mx = mouseX();
    let my = mouseY();
    for _ in 0..30 {
        px.push(mx);
        py.push(my);
        pvx.push(random_range(-6.0, 6.0));
        pvy.push(random_range(-8.0, -2.0));
        plife.push(1.0);
    }
}
