// Interactive Colors
// Click to randomise the background; press any key to reset.
// Exercises: mouse_pressed, key_pressed, random, fill, text, textSize.

let mut bg: f32 = 0.0;
let mut r: f32 = 200.0;
let mut g: f32 = 100.0;
let mut b: f32 = 50.0;

fn setup() {
    size(600, 400);
    textSize(18.0);
}

fn draw() {
    background(bg);
    fill_rgb(r, g, b);
    ellipse(mouseX(), mouseY(), 60.0, 60.0);

    fill(220.0);
    text("Click to randomise   |   any key to reset", 20.0, 30.0);
}

fn mouse_pressed() {
    bg = random(255.0);
    r  = random(255.0);
    g  = random(255.0);
    b  = random(255.0);
}

fn key_pressed() {
    bg = 0.0;
    r  = 200.0;
    g  = 100.0;
    b  = 50.0;
}
