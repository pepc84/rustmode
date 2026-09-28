// KeyboardMove – move a square with the arrow keys; press R to reset.
// Demonstrates: key_code(), UP / DOWN / LEFT / RIGHT constants,
//               key_pressed global, key_typed(), rect, fill, text.

let mut x: f32 = 0.0;
let mut y: f32 = 0.0;
let speed: f32 = 3.0;

fn setup() {
    size(600, 600);
    frameRate(60.0);
    x = width() / 2.0;
    y = height() / 2.0;
}

fn draw() {
    background(30.0);

    // Arrow-key movement (held down = continuous).
    if keyPressed() {
        let kc = key_code();
        if kc == LEFT  { x -= speed; }
        if kc == RIGHT { x += speed; }
        if kc == UP    { y -= speed; }
        if kc == DOWN  { y += speed; }
    }

    // Keep the square on screen.
    x = constrain(x, 20.0, width() - 20.0);
    y = constrain(y, 20.0, height() - 20.0);

    // Draw square.
    fill(80.0, 160.0, 255.0);
    no_stroke();
    rect(x - 20.0, y - 20.0, 40.0, 40.0);

    // HUD.
    fill(200.0);
    text_size(13.0);
    text("Arrow keys: move   R: reset", 10.0, 16.0);
}

fn key_typed() {
    if key() == 'r' || key() == 'R' {
        x = width() / 2.0;
        y = height() / 2.0;
    }
    if key_code() == ESCAPE {
        exit();
    }
}
