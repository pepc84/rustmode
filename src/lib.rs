//! # processing
//!
//! Processing-style API for RustMode sketches.
//!
//! Generated sketches do `use processing::*;` and get all Processing drawing,
//! math, input, and lifecycle functions in scope — both snake_case and
//! camelCase naming conventions.
//!
//! Backend: software pixel-buffer renderer via minifb (native) or stub (wasm).

// The camelCase aliases (mouseX, noFill, frameRate, …) are intentional.
#![allow(non_snake_case)]

use parking_lot::RwLock;
use std::sync::{atomic::{AtomicU32, Ordering}, OnceLock};

// ─────────────────────────────────────────────────────────────────────────────
// Canvas dimensions (shared between glfw runner and render module)
// ─────────────────────────────────────────────────────────────────────────────

static CANVAS_W: AtomicU32 = AtomicU32::new(100);
static CANVAS_H: AtomicU32 = AtomicU32::new(100);

// Pixel buffer: 0xRRGGBB packed as u32 (minifb native format)
static PIXEL_BUF: OnceLock<parking_lot::Mutex<Vec<u32>>> = OnceLock::new();
fn pixel_buf() -> &'static parking_lot::Mutex<Vec<u32>> {
    PIXEL_BUF.get_or_init(|| {
        let w = CANVAS_W.load(Ordering::Relaxed) as usize;
        let h = CANVAS_H.load(Ordering::Relaxed) as usize;
        parking_lot::Mutex::new(vec![0xFF_20_20_20u32; w * h])
    })
}

fn resize_canvas(w: u32, h: u32) {
    CANVAS_W.store(w, Ordering::Relaxed);
    CANVAS_H.store(h, Ordering::Relaxed);
    let mut buf = pixel_buf().lock();
    buf.resize((w * h) as usize, 0xFF_20_20_20u32);
}

// ─────────────────────────────────────────────────────────────────────────────
// Core types
// ─────────────────────────────────────────────────────────────────────────────

mod core {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Color { pub r: f32, pub g: f32, pub b: f32, pub a: f32 }

    impl Color {
        pub fn from_gray(v: f32) -> Self { Self { r: v, g: v, b: v, a: 1.0 } }
        pub fn from_rgb(r: f32, g: f32, b: f32) -> Self { Self { r, g, b, a: 1.0 } }
        pub fn from_rgba(r: f32, g: f32, b: f32, a: f32) -> Self { Self { r, g, b, a } }

        pub fn to_u32(self) -> u32 {
            let r = (self.r.clamp(0.0, 1.0) * 255.0) as u32;
            let g = (self.g.clamp(0.0, 1.0) * 255.0) as u32;
            let b = (self.b.clamp(0.0, 1.0) * 255.0) as u32;
            (r << 16) | (g << 8) | b
        }
    }

    pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
        Color {
            r: a.r + (b.r - a.r) * t,
            g: a.g + (b.g - a.g) * t,
            b: a.b + (b.b - a.b) * t,
            a: a.a + (b.a - a.a) * t,
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Default)]
    pub struct PVector { pub x: f32, pub y: f32, pub z: f32 }

    impl PVector {
        pub fn new(x: f32, y: f32) -> Self { Self { x, y, z: 0.0 } }
        pub fn new3(x: f32, y: f32, z: f32) -> Self { Self { x, y, z } }
        pub fn mag(&self) -> f32 { (self.x*self.x + self.y*self.y + self.z*self.z).sqrt() }
        pub fn normalize(&mut self) {
            let m = self.mag();
            if m > 0.0 { self.x /= m; self.y /= m; self.z /= m; }
        }
    }

    // Pseudo-noise (sin hash). Good enough for most sketches.
    pub fn noise1(x: f32) -> f32 { ((x * 127.1).sin() * 43758.5453).fract().abs() }
    pub fn noise2(x: f32, y: f32) -> f32 {
        ((x * 127.1 + y * 311.7).sin() * 43758.5453).fract().abs()
    }

    // Simple xorshift RNG
    use std::sync::{Mutex, OnceLock};
    static RNG: OnceLock<Mutex<u64>> = OnceLock::new();
    fn rng() -> &'static Mutex<u64> { RNG.get_or_init(|| Mutex::new(12345)) }
    fn next(s: &mut u64) -> u64 { *s ^= *s<<13; *s ^= *s>>7; *s ^= *s<<17; *s }

    pub fn random_seed(seed: u64) { *rng().lock().unwrap() = seed; }
    pub fn random_f32(low: f32, high: f32) -> f32 {
        let mut s = rng().lock().unwrap();
        let t = next(&mut s) as f32 / u64::MAX as f32;
        low + (high - low) * t
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Software rasterizer
// ─────────────────────────────────────────────────────────────────────────────

mod render {
    // ── begin_shape / vertex / end_shape (inserted by shapes patch) ─────────
    // Vertices are stored untransformed; fills go through tfm + fill_tri,
    // strokes through draw_line, so both follow the current matrix and style.
    struct ShapeState { kind: u8, pts: Vec<(f32, f32)>, curve: Vec<(f32, f32)>, active: bool }
    fn shape_state() -> &'static parking_lot::Mutex<ShapeState> {
        static S: std::sync::OnceLock<parking_lot::Mutex<ShapeState>> = std::sync::OnceLock::new();
        S.get_or_init(|| parking_lot::Mutex::new(ShapeState { kind: 0, pts: Vec::new(), curve: Vec::new(), active: false }))
    }
    pub fn shape_begin(kind: u8) {
        let mut s = shape_state().lock();
        s.kind = kind; s.pts.clear(); s.curve.clear(); s.active = true;
    }
    pub fn shape_vertex(x: f32, y: f32) {
        let mut s = shape_state().lock();
        s.curve.clear();
        s.pts.push((x, y));
    }
    pub fn shape_bezier_vertex(cx1: f32, cy1: f32, cx2: f32, cy2: f32, x: f32, y: f32) {
        let mut s = shape_state().lock();
        let (x0, y0) = match s.pts.last() { Some(p) => *p, None => { s.pts.push((x, y)); return; } };
        for i in 1..=24 {
            let t = i as f32 / 24.0; let u = 1.0 - t;
            let px = u*u*u*x0 + 3.0*u*u*t*cx1 + 3.0*u*t*t*cx2 + t*t*t*x;
            let py = u*u*u*y0 + 3.0*u*u*t*cy1 + 3.0*u*t*t*cy2 + t*t*t*y;
            s.pts.push((px, py));
        }
    }
    pub fn shape_quadratic_vertex(cx: f32, cy: f32, x: f32, y: f32) {
        let mut s = shape_state().lock();
        let (x0, y0) = match s.pts.last() { Some(p) => *p, None => { s.pts.push((x, y)); return; } };
        for i in 1..=16 {
            let t = i as f32 / 16.0; let u = 1.0 - t;
            s.pts.push((u*u*x0 + 2.0*u*t*cx + t*t*x, u*u*y0 + 2.0*u*t*cy + t*t*y));
        }
    }
    /// Catmull-Rom, like Processing: the first and last curve_vertex are control points only.
    pub fn shape_curve_vertex(x: f32, y: f32) {
        let mut s = shape_state().lock();
        s.curve.push((x, y));
        let n = s.curve.len();
        if n < 4 { return; }
        let (p0, p1, p2, p3) = (s.curve[n-4], s.curve[n-3], s.curve[n-2], s.curve[n-1]);
        let start = if n == 4 { 0 } else { 1 };
        for i in start..=12 {
            let t = i as f32 / 12.0; let t2 = t*t; let t3 = t2*t;
            let f = |a: f32, b: f32, c: f32, d: f32| 0.5 * ((2.0*b) + (-a + c)*t + (2.0*a - 5.0*b + 4.0*c - d)*t2 + (-a + 3.0*b - 3.0*c + d)*t3);
            let pt = (f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1));
            s.pts.push(pt);
        }
    }
    pub fn shape_end(close: bool) {
        let (kind, pts) = {
            let mut s = shape_state().lock();
            if !s.active { return; }
            s.active = false;
            (s.kind, std::mem::take(&mut s.pts))
        };
        let (has_fill, has_stroke) = with_style(|s| (s.fill.is_some(), s.stroke.is_some()));
        let tri = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
            if has_fill { shape_fill_tri(a, b, c); }
            if has_stroke { draw_line(a.0, a.1, b.0, b.1); draw_line(b.0, b.1, c.0, c.1); draw_line(c.0, c.1, a.0, a.1); }
        };
        match kind {
            1 => for p in &pts { draw_point(p.0, p.1); },                       // POINTS
            2 => if has_stroke { for w in pts.chunks_exact(2) { draw_line(w[0].0, w[0].1, w[1].0, w[1].1); } }, // LINES
            3 => for w in pts.chunks_exact(3) { tri(w[0], w[1], w[2]); },        // TRIANGLES
            4 => for i in 2..pts.len() { tri(pts[i-2], pts[i-1], pts[i]); },     // TRIANGLE_STRIP
            5 => for i in 2..pts.len() { tri(pts[0], pts[i-1], pts[i]); },       // TRIANGLE_FAN
            6 => for q in pts.chunks_exact(4) { shape_polygon(q, true, has_fill, has_stroke); }, // QUADS
            7 => for i in (3..pts.len()).step_by(2) {                            // QUAD_STRIP
                     let q = [pts[i-3], pts[i-2], pts[i], pts[i-1]];
                     shape_polygon(&q, true, has_fill, has_stroke);
                 },
            _ => shape_polygon(&pts, close, has_fill, has_stroke),              // polygon
        }
    }
    fn shape_fill_tri(a: (f32, f32), b: (f32, f32), c: (f32, f32)) {
        let (cw, ch) = canvas_dims();
        let fill = with_style(|s| s.fill);
        if let Some(fc) = fill {
            let (ax, ay) = tfm(a.0, a.1);
            let (bx, by) = tfm(b.0, b.1);
            let (cx, cy) = tfm(c.0, c.1);
            let mut buf = pixel_buf().lock();
            fill_tri(&mut buf, cw, ch, ax, ay, bx, by, cx, cy, fc);
        }
    }
    /// Fill any simple polygon (concave too) by ear clipping, then stroke its outline.
    fn shape_polygon(pts: &[(f32, f32)], close: bool, has_fill: bool, has_stroke: bool) {
        let n = pts.len();
        if has_fill && n >= 3 {
            let area: f32 = (0..n).map(|i| { let (a, b) = (pts[i], pts[(i+1)%n]); a.0*b.1 - b.0*a.1 }).sum();
            let ccw = area > 0.0;
            let cross = |o: (f32,f32), a: (f32,f32), b: (f32,f32)| (a.0-o.0)*(b.1-o.1) - (a.1-o.1)*(b.0-o.0);
            let mut idx: Vec<usize> = (0..n).collect();
            let mut guard = 0;
            while idx.len() > 3 && guard < n * n {
                guard += 1;
                let m = idx.len();
                let mut clipped = false;
                for k in 0..m {
                    let (ia, ib, ic) = (idx[(k+m-1)%m], idx[k], idx[(k+1)%m]);
                    let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
                    let c1 = cross(a, b, c);
                    if (c1 > 0.0) != ccw || c1 == 0.0 { continue; }
                    let inside = idx.iter().any(|&j| {
                        if j == ia || j == ib || j == ic { return false; }
                        let p = pts[j];
                        let (d1, d2, d3) = (cross(a, b, p), cross(b, c, p), cross(c, a, p));
                        if ccw { d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0 } else { d1 <= 0.0 && d2 <= 0.0 && d3 <= 0.0 }
                    });
                    if inside { continue; }
                    shape_fill_tri(a, b, c);
                    idx.remove(k);
                    clipped = true;
                    break;
                }
                if !clipped { break; } // self-intersecting; fan the rest below
            }
            for i in 2..idx.len() { shape_fill_tri(pts[idx[0]], pts[idx[i-1]], pts[idx[i]]); }
        }
        if has_stroke && n >= 2 {
            for i in 1..n { draw_line(pts[i-1].0, pts[i-1].1, pts[i].0, pts[i].1); }
            if close { draw_line(pts[n-1].0, pts[n-1].1, pts[0].0, pts[0].1); }
        }
    }

    use super::{CANVAS_W, CANVAS_H, pixel_buf};
    use super::core::Color;
    use std::sync::atomic::Ordering;

    // ── Style state ────────────────────────────────────────────────────────────

    #[derive(Clone)]
    struct Style {
        fill:         Option<Color>,
        stroke:       Option<Color>,
        stroke_weight: f32,
    }
    impl Default for Style {
        fn default() -> Self {
            Self {
                fill:   Some(Color::from_gray(1.0)),
                stroke: Some(Color::from_gray(0.0)),
                stroke_weight: 1.0,
            }
        }
    }

    thread_local! {
        static STYLE_STACK: std::cell::RefCell<Vec<Style>> =
            std::cell::RefCell::new(vec![Style::default()]);
    }
    fn with_style<R>(f: impl FnOnce(&Style) -> R) -> R {
        STYLE_STACK.with(|s| { let stack = s.borrow(); f(stack.last().unwrap()) })
    }
    fn with_style_mut<R>(f: impl FnOnce(&mut Style) -> R) -> R {
        STYLE_STACK.with(|s| { let mut stack = s.borrow_mut(); f(stack.last_mut().unwrap()) })
    }

    // ── Transform matrix stack (2D affine: [a,b,c,d,tx,ty]) ──────────────────

    type Mat = [f32; 6]; // [a, b, c, d, tx, ty]

    fn identity() -> Mat { [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] }

    fn mat_mul(a: &Mat, b: &Mat) -> Mat {
        [
            a[0]*b[0] + a[2]*b[1],  a[1]*b[0] + a[3]*b[1],
            a[0]*b[2] + a[2]*b[3],  a[1]*b[2] + a[3]*b[3],
            a[0]*b[4] + a[2]*b[5] + a[4],
            a[1]*b[4] + a[3]*b[5] + a[5],
        ]
    }

    thread_local! {
        static MATRIX_STACK: std::cell::RefCell<Vec<Mat>> =
            std::cell::RefCell::new(vec![identity()]);
    }
    fn current_matrix() -> Mat {
        MATRIX_STACK.with(|s| *s.borrow().last().unwrap())
    }

    // ── Pixel operations ───────────────────────────────────────────────────────

    fn put(buf: &mut Vec<u32>, w: usize, h: usize, x: i32, y: i32, col: Color) {
        if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 { return; }
        let idx = y as usize * w + x as usize;
        if col.a >= 1.0 {
            buf[idx] = col.to_u32();
        } else {
            let a = col.a;
            let dst = buf[idx];
            let dr = ((dst >> 16) & 0xFF) as f32 / 255.0;
            let dg = ((dst >> 8) & 0xFF) as f32 / 255.0;
            let db = (dst & 0xFF) as f32 / 255.0;
            let blended = Color::from_rgb(
                dr + (col.r - dr) * a,
                dg + (col.g - dg) * a,
                db + (col.b - db) * a,
            );
            buf[idx] = blended.to_u32();
        }
    }

    // Bresenham line
    fn bline(buf: &mut Vec<u32>, w: usize, h: usize,
             x0: i32, y0: i32, x1: i32, y1: i32, col: Color) {
        let dx = (x1-x0).abs(); let dy = (y1-y0).abs();
        let sx = if x0 < x1 { 1i32 } else { -1 };
        let sy = if y0 < y1 { 1i32 } else { -1 };
        let mut err = dx - dy;
        let (mut x, mut y) = (x0, y0);
        loop {
            put(buf, w, h, x, y, col);
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 > -dy { err -= dy; x += sx; }
            if e2 <  dx { err += dx; y += sy; }
        }
    }

    // Filled triangle (flat scan-line)
    fn fill_tri(buf: &mut Vec<u32>, w: usize, h: usize,
                x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, col: Color) {
        let (mut ax, mut ay) = (x0 as i32, y0 as i32);
        let (mut bx, mut by) = (x1 as i32, y1 as i32);
        let (mut cx, mut cy) = (x2 as i32, y2 as i32);
        // sort by y
        if ay > by { std::mem::swap(&mut ax, &mut bx); std::mem::swap(&mut ay, &mut by); }
        if ay > cy { std::mem::swap(&mut ax, &mut cx); std::mem::swap(&mut ay, &mut cy); }
        if by > cy { std::mem::swap(&mut bx, &mut cx); std::mem::swap(&mut by, &mut cy); }
        let total_h = cy - ay;
        if total_h == 0 { return; }
        for i in 0..total_h {
            let second_half = i >= by - ay || by == ay;
            let seg_h = if second_half { cy - by } else { by - ay };
            if seg_h == 0 { continue; }
            let alpha = i as f32 / total_h as f32;
            let beta  = if second_half {
                (i - (by - ay)) as f32 / seg_h as f32
            } else {
                i as f32 / seg_h as f32
            };
            let mut lx = ax as f32 + (cx - ax) as f32 * alpha;
            let mut rx = if second_half {
                bx as f32 + (cx - bx) as f32 * beta
            } else {
                ax as f32 + (bx - ax) as f32 * beta
            };
            if lx > rx { std::mem::swap(&mut lx, &mut rx); }
            let py = ay + i;
            for px in (lx as i32)..=(rx as i32) {
                put(buf, w, h, px, py, col);
            }
        }
    }

    // ── Public drawing API ─────────────────────────────────────────────────────

    pub fn begin_draw() {}
    pub fn end_draw()   {}
    pub fn present()    {}

    pub fn background_gray(v: f32) {
        let col = Color::from_gray(v);
        let p = col.to_u32();
        let mut buf = pixel_buf().lock();
        buf.fill(p);
    }
    pub fn background_rgb(r: f32, g: f32, b: f32) {
        let col = Color::from_rgb(r, g, b);
        let p = col.to_u32();
        let mut buf = pixel_buf().lock();
        buf.fill(p);
    }
    pub fn background_rgba(r: f32, g: f32, b: f32, _a: f32) { background_rgb(r, g, b); }
    pub fn background_color(c: Color) {
        let p = c.to_u32();
        let mut buf = pixel_buf().lock();
        buf.fill(p);
    }

    pub fn set_fill_gray(v: f32)                    { with_style_mut(|s| s.fill = Some(Color::from_gray(v))); }
    pub fn set_fill_rgb(r: f32, g: f32, b: f32)     { with_style_mut(|s| s.fill = Some(Color::from_rgb(r,g,b))); }
    pub fn set_fill_rgba(r: f32, g: f32, b: f32, a: f32) { with_style_mut(|s| s.fill = Some(Color::from_rgba(r,g,b,a))); }
    pub fn set_fill_none()                           { with_style_mut(|s| s.fill = None); }
    pub fn set_fill_color(c: Color)                  { with_style_mut(|s| s.fill = Some(c)); }

    pub fn set_stroke_gray(v: f32)                   { with_style_mut(|s| s.stroke = Some(Color::from_gray(v))); }
    pub fn set_stroke_rgb(r: f32, g: f32, b: f32)    { with_style_mut(|s| s.stroke = Some(Color::from_rgb(r,g,b))); }
    pub fn set_stroke_rgba(r: f32, g: f32, b: f32, a: f32) { with_style_mut(|s| s.stroke = Some(Color::from_rgba(r,g,b,a))); }
    pub fn set_stroke_none()                          { with_style_mut(|s| s.stroke = None); }
    pub fn set_stroke_color(c: Color)                 { with_style_mut(|s| s.stroke = Some(c)); }
    pub fn set_stroke_weight(w: f32)                  { with_style_mut(|s| s.stroke_weight = w); }

    fn canvas_dims() -> (usize, usize) {
        (CANVAS_W.load(Ordering::Relaxed) as usize, CANVAS_H.load(Ordering::Relaxed) as usize)
    }

    fn tfm(x: f32, y: f32) -> (f32, f32) {
        let m = current_matrix();
        (m[0]*x + m[2]*y + m[4], m[1]*x + m[3]*y + m[5])
    }

    pub fn draw_ellipse(cx: f32, cy: f32, w: f32, h: f32) {
        let (cw, ch) = canvas_dims();
        let (fill, stroke, sw) = with_style(|s| (s.fill, s.stroke, s.stroke_weight));
        let (tcx, tcy) = tfm(cx, cy);
        let rx = w * 0.5; let ry = h * 0.5;
        let steps = ((rx.max(ry) * std::f32::consts::TAU) as usize).max(32);
        if let Some(fc) = fill {
            // rasterise as filled polygon via scan-line over bounding box
            let y0 = (tcy - ry) as i32; let y1 = (tcy + ry) as i32;
            let mut buf = pixel_buf().lock();
            for py in y0..=y1 {
                let dy = (py as f32 - tcy) / ry;
                if dy.abs() > 1.0 { continue; }
                let dx = (1.0 - dy*dy).sqrt() * rx;
                for px in ((tcx - dx) as i32)..=((tcx + dx) as i32) {
                    put(&mut buf, cw, ch, px, py, fc);
                }
            }
            drop(buf);
        }
        if let Some(sc) = stroke {
            let mut buf = pixel_buf().lock();
            let thick = sw.max(1.0) as i32;
            for i in 0..steps {
                let t0 = i as f32 / steps as f32 * std::f32::consts::TAU;
                let t1 = (i+1) as f32 / steps as f32 * std::f32::consts::TAU;
                let (ax, ay) = (tcx + t0.cos() * rx, tcy + t0.sin() * ry);
                let (bx, by) = (tcx + t1.cos() * rx, tcy + t1.sin() * ry);
                for _ in 0..thick {
                    bline(&mut buf, cw, ch, ax as i32, ay as i32, bx as i32, by as i32, sc);
                }
            }
        }
    }

    pub fn draw_rect(x: f32, y: f32, w: f32, h: f32) {
        let (cw, ch) = canvas_dims();
        let (fill, stroke, sw) = with_style(|s| (s.fill, s.stroke, s.stroke_weight));
        // Transform all four corners so rotation/scale in the matrix stack work correctly.
        let c = [tfm(x, y), tfm(x+w, y), tfm(x+w, y+h), tfm(x, y+h)];
        if let Some(fc) = fill {
            let mut buf = pixel_buf().lock();
            fill_tri(&mut buf, cw, ch, c[0].0, c[0].1, c[1].0, c[1].1, c[2].0, c[2].1, fc);
            fill_tri(&mut buf, cw, ch, c[0].0, c[0].1, c[2].0, c[2].1, c[3].0, c[3].1, fc);
        }
        if let Some(sc) = stroke {
            let mut buf = pixel_buf().lock();
            let thick = sw.max(1.0) as i32;
            let pts = [c[0], c[1], c[2], c[3], c[0]];
            for i in 0..4 {
                let (ax,ay) = (pts[i].0 as i32, pts[i].1 as i32);
                let (bx,by) = (pts[i+1].0 as i32, pts[i+1].1 as i32);
                for _ in 0..thick { bline(&mut buf, cw, ch, ax, ay, bx, by, sc); }
            }
        }
    }

    pub fn draw_rect_rounded(x: f32, y: f32, w: f32, h: f32, r: f32) {
        // Clamp radius so it fits inside the rectangle.
        let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
        if r < 1.0 { return draw_rect(x, y, w, h); }

        let (cw, ch) = canvas_dims();
        let (fill, stroke, sw) = with_style(|s| (s.fill, s.stroke, s.stroke_weight));

        // Fill: scan-line over the bounding box, include pixel if inside rounded rect.
        if let Some(fc) = fill {
            let (tx0, ty0) = tfm(x, y);
            let (tx1, ty1) = tfm(x + w, y + h);
            // Only handles axis-aligned case; transform may have rotation but corners
            // are the only rounded part — approximate by transforming the 4 circle centres.
            let corners = [
                tfm(x + r, y + r),       // top-left
                tfm(x + w - r, y + r),   // top-right
                tfm(x + w - r, y + h - r), // bottom-right
                tfm(x + r, y + h - r),   // bottom-left
            ];
            let (sx, sy) = (tx0.min(tx1) as i32, ty0.min(ty1) as i32);
            let (ex, ey) = (tx0.max(tx1) as i32, ty0.max(ty1) as i32);
            let mut buf = pixel_buf().lock();
            for py in sy..=ey {
                for px in sx..=ex {
                    let pfx = px as f32 + 0.5;
                    let pfy = py as f32 + 0.5;
                    // In-corner regions: check circle; elsewhere always include.
                    let in_shape = [
                        (pfx < corners[0].0 && pfy < corners[0].1, corners[0]),
                        (pfx > corners[1].0 && pfy < corners[1].1, corners[1]),
                        (pfx > corners[2].0 && pfy > corners[2].1, corners[2]),
                        (pfx < corners[3].0 && pfy > corners[3].1, corners[3]),
                    ].iter().all(|(is_corner, c)| {
                        !is_corner || {
                            let ddx = pfx - c.0; let ddy = pfy - c.1;
                            ddx*ddx + ddy*ddy <= r*r
                        }
                    });
                    if in_shape { put(&mut buf, cw, ch, px, py, fc); }
                }
            }
        }

        // Stroke: arc at each corner + straight segments between them.
        if let Some(sc) = stroke {
            let thick = sw.max(1.0) as i32;
            let arc_steps = (r * 8.0) as usize + 8;
            let (c0, c1, c2, c3) = (
                tfm(x + r,     y + r),
                tfm(x + w - r, y + r),
                tfm(x + w - r, y + h - r),
                tfm(x + r,     y + h - r),
            );
            let mut buf = pixel_buf().lock();
            // Four corner arcs
            let arcs = [
                (c0, std::f32::consts::PI, 1.5 * std::f32::consts::PI),
                (c1, 1.5 * std::f32::consts::PI, 2.0 * std::f32::consts::PI),
                (c2, 0.0_f32, 0.5 * std::f32::consts::PI),
                (c3, 0.5 * std::f32::consts::PI, std::f32::consts::PI),
            ];
            for (c, a0, a1) in arcs {
                for i in 0..arc_steps {
                    let t0 = a0 + i as f32 / arc_steps as f32 * (a1 - a0);
                    let t1 = a0 + (i+1) as f32 / arc_steps as f32 * (a1 - a0);
                    let (ax, ay) = (c.0 + t0.cos() * r, c.1 + t0.sin() * r);
                    let (bx, by) = (c.0 + t1.cos() * r, c.1 + t1.sin() * r);
                    for _ in 0..thick {
                        bline(&mut buf, cw, ch, ax as i32, ay as i32, bx as i32, by as i32, sc);
                    }
                }
            }
            // Four straight edges between the arc endpoints
            let edges = [
                ((c0.0, c0.1 - r), (c1.0, c1.1 - r)),  // top
                ((c1.0 + r, c1.1), (c2.0 + r, c2.1)),  // right
                ((c3.0, c3.1 + r), (c2.0, c2.1 + r)),  // bottom
                ((c0.0 - r, c0.1), (c3.0 - r, c3.1)),  // left
            ];
            for ((ax, ay), (bx, by)) in edges {
                for _ in 0..thick {
                    bline(&mut buf, cw, ch, ax as i32, ay as i32, bx as i32, by as i32, sc);
                }
            }
        }
    }

    pub fn draw_triangle(x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) {
        let (cw, ch) = canvas_dims();
        let (fill, stroke, sw) = with_style(|s| (s.fill, s.stroke, s.stroke_weight));
        let (tx1,ty1) = tfm(x1,y1);
        let (tx2,ty2) = tfm(x2,y2);
        let (tx3,ty3) = tfm(x3,y3);
        if let Some(fc) = fill {
            let mut buf = pixel_buf().lock();
            fill_tri(&mut buf, cw, ch, tx1, ty1, tx2, ty2, tx3, ty3, fc);
        }
        if let Some(sc) = stroke {
            let mut buf = pixel_buf().lock();
            let thick = sw.max(1.0) as i32;
            let pts = [(tx1,ty1),(tx2,ty2),(tx3,ty3),(tx1,ty1)];
            for i in 0..3 {
                let (ax,ay) = (pts[i].0 as i32, pts[i].1 as i32);
                let (bx,by) = (pts[i+1].0 as i32, pts[i+1].1 as i32);
                for _ in 0..thick {
                    bline(&mut buf, cw, ch, ax, ay, bx, by, sc);
                }
            }
        }
    }

    pub fn draw_line(x1: f32, y1: f32, x2: f32, y2: f32) {
        let (cw, ch) = canvas_dims();
        let (sc, sw) = with_style(|s| (s.stroke, s.stroke_weight));
        if let Some(c) = sc {
            let (tx1,ty1) = tfm(x1,y1);
            let (tx2,ty2) = tfm(x2,y2);
            let mut buf = pixel_buf().lock();
            if sw <= 1.5 {
                bline(&mut buf, cw, ch, tx1 as i32, ty1 as i32, tx2 as i32, ty2 as i32, c);
            } else {
                // Thick line: fill a quad perpendicular to the line direction.
                let dx = tx2 - tx1;
                let dy = ty2 - ty1;
                let len = (dx*dx + dy*dy).sqrt().max(0.001);
                let nx = -dy / len * sw * 0.5;
                let ny =  dx / len * sw * 0.5;
                fill_tri(&mut buf, cw, ch,
                    tx1-nx, ty1-ny, tx1+nx, ty1+ny, tx2+nx, ty2+ny, c);
                fill_tri(&mut buf, cw, ch,
                    tx1-nx, ty1-ny, tx2+nx, ty2+ny, tx2-nx, ty2-ny, c);
            }
        }
    }

    pub fn draw_point(x: f32, y: f32) {
        let (cw, ch) = canvas_dims();
        let (sc, sw) = with_style(|s| (s.stroke, s.stroke_weight));
        if let Some(c) = sc {
            let (tx,ty) = tfm(x,y);
            let mut buf = pixel_buf().lock();
            if sw <= 1.5 {
                put(&mut buf, cw, ch, tx as i32, ty as i32, c);
            } else {
                // Draw a filled circle of radius sw/2 centred on the point.
                let r = sw * 0.5;
                let y0 = (ty - r) as i32; let y1 = (ty + r) as i32;
                for py in y0..=y1 {
                    let dy = py as f32 - ty;
                    if dy.abs() > r { continue; }
                    let dx = (r*r - dy*dy).sqrt();
                    for px in ((tx - dx) as i32)..=((tx + dx) as i32) {
                        put(&mut buf, cw, ch, px, py, c);
                    }
                }
            }
        }
    }

    pub fn draw_quad(x1: f32, y1: f32, x2: f32, y2: f32,
                     x3: f32, y3: f32, x4: f32, y4: f32) {
        let (cw, ch) = canvas_dims();
        let (fill, stroke, sw) = with_style(|s| (s.fill, s.stroke, s.stroke_weight));
        let (tx1,ty1) = tfm(x1,y1);
        let (tx2,ty2) = tfm(x2,y2);
        let (tx3,ty3) = tfm(x3,y3);
        let (tx4,ty4) = tfm(x4,y4);
        if let Some(fc) = fill {
            let mut buf = pixel_buf().lock();
            fill_tri(&mut buf, cw, ch, tx1, ty1, tx2, ty2, tx3, ty3, fc);
            fill_tri(&mut buf, cw, ch, tx1, ty1, tx3, ty3, tx4, ty4, fc);
        }
        if let Some(sc) = stroke {
            let mut buf = pixel_buf().lock();
            let thick = sw.max(1.0) as i32;
            let pts = [(tx1,ty1),(tx2,ty2),(tx3,ty3),(tx4,ty4),(tx1,ty1)];
            for i in 0..4 {
                let (ax,ay) = (pts[i].0 as i32, pts[i].1 as i32);
                let (bx,by) = (pts[i+1].0 as i32, pts[i+1].1 as i32);
                for _ in 0..thick { bline(&mut buf, cw, ch, ax, ay, bx, by, sc); }
            }
        }
    }

    pub fn draw_arc(cx: f32, cy: f32, w: f32, h: f32, start: f32, stop: f32) {
        let (cw, ch) = canvas_dims();
        let (fill, stroke, sw) = with_style(|s| (s.fill, s.stroke, s.stroke_weight));
        let (tcx, tcy) = tfm(cx, cy);
        let rx = w * 0.5; let ry = h * 0.5;
        let angle_span = (stop - start).abs();
        let steps = ((rx.max(ry) * angle_span) as usize).max(16);

        // Fill: pie slices via triangles from the centre.
        if let Some(fc) = fill {
            let mut buf = pixel_buf().lock();
            for i in 0..steps {
                let t0 = start + i as f32 / steps as f32 * (stop - start);
                let t1 = start + (i+1) as f32 / steps as f32 * (stop - start);
                let (ax, ay) = (tcx + t0.cos() * rx, tcy + t0.sin() * ry);
                let (bx, by) = (tcx + t1.cos() * rx, tcy + t1.sin() * ry);
                fill_tri(&mut buf, cw, ch, tcx, tcy, ax, ay, bx, by, fc);
            }
        }

        // Stroke: arc outline + two radial lines closing the pie.
        if let Some(sc) = stroke {
            let thick = sw.max(1.0) as i32;
            let mut buf = pixel_buf().lock();
            for i in 0..steps {
                let t0 = start + i as f32 / steps as f32 * (stop - start);
                let t1 = start + (i+1) as f32 / steps as f32 * (stop - start);
                let (ax, ay) = (tcx + t0.cos() * rx, tcy + t0.sin() * ry);
                let (bx, by) = (tcx + t1.cos() * rx, tcy + t1.sin() * ry);
                for _ in 0..thick {
                    bline(&mut buf, cw, ch, ax as i32, ay as i32, bx as i32, by as i32, sc);
                }
            }
            // Close with radial lines only when fill is also set (pie mode).
            if fill.is_some() {
                let (ax, ay) = (tcx + start.cos() * rx, tcy + start.sin() * ry);
                let (bx, by) = (tcx + stop.cos()  * rx, tcy + stop.sin()  * ry);
                for _ in 0..thick {
                    bline(&mut buf, cw, ch, tcx as i32, tcy as i32, ax as i32, ay as i32, sc);
                    bline(&mut buf, cw, ch, tcx as i32, tcy as i32, bx as i32, by as i32, sc);
                }
            }
        }
    }

    // ── Transforms ────────────────────────────────────────────────────────────

    pub fn push_matrix() {
        MATRIX_STACK.with(|s| {
            let top = *s.borrow().last().unwrap();
            s.borrow_mut().push(top);
        });
    }
    pub fn pop_matrix() {
        MATRIX_STACK.with(|s| { let mut st = s.borrow_mut(); if st.len() > 1 { st.pop(); } });
    }
    pub fn translate(tx: f32, ty: f32, _tz: f32) {
        MATRIX_STACK.with(|s| {
            let mut st = s.borrow_mut();
            let top = st.last_mut().unwrap();
            let t: Mat = [1.0, 0.0, 0.0, 1.0, tx, ty];
            *top = mat_mul(top, &t);
        });
    }
    pub fn rotate_z(angle: f32) {
        MATRIX_STACK.with(|s| {
            let mut st = s.borrow_mut();
            let top = st.last_mut().unwrap();
            let (cos, sin) = (angle.cos(), angle.sin());
            let r: Mat = [cos, sin, -sin, cos, 0.0, 0.0];
            *top = mat_mul(top, &r);
        });
    }
    pub fn scale(sx: f32, sy: f32, _sz: f32) {
        MATRIX_STACK.with(|s| {
            let mut st = s.borrow_mut();
            let top = st.last_mut().unwrap();
            let sc: Mat = [sx, 0.0, 0.0, sy, 0.0, 0.0];
            *top = mat_mul(top, &sc);
        });
    }

    // ── Style stack ───────────────────────────────────────────────────────────

    pub fn push_style() {
        STYLE_STACK.with(|s| {
            let top = s.borrow().last().unwrap().clone();
            s.borrow_mut().push(top);
        });
    }
    pub fn pop_style() {
        STYLE_STACK.with(|s| { let mut st = s.borrow_mut(); if st.len() > 1 { st.pop(); } });
    }

    // ── Text rendering ────────────────────────────────────────────────────────

    thread_local! {
        static TEXT_SIZE: std::cell::RefCell<f32> = std::cell::RefCell::new(12.0);
    }

    // 5×8 bitmap font for printable ASCII (0x20–0x7F).
    // Each entry: [col0..col4]; each byte = 8 rows, bit 0 = top row.
    static FONT5X8: [[u8; 5]; 96] = [
        [0x00,0x00,0x00,0x00,0x00], // ' '
        [0x00,0x00,0x5F,0x00,0x00], // '!'
        [0x00,0x07,0x00,0x07,0x00], // '"'
        [0x14,0x7F,0x14,0x7F,0x14], // '#'
        [0x24,0x2A,0x7F,0x2A,0x12], // '$'
        [0x23,0x13,0x08,0x64,0x62], // '%'
        [0x36,0x49,0x55,0x22,0x50], // '&'
        [0x00,0x05,0x03,0x00,0x00], // '\''
        [0x00,0x1C,0x22,0x41,0x00], // '('
        [0x00,0x41,0x22,0x1C,0x00], // ')'
        [0x14,0x08,0x3E,0x08,0x14], // '*'
        [0x08,0x08,0x3E,0x08,0x08], // '+'
        [0x00,0x50,0x30,0x00,0x00], // ','
        [0x08,0x08,0x08,0x08,0x08], // '-'
        [0x00,0x60,0x60,0x00,0x00], // '.'
        [0x20,0x10,0x08,0x04,0x02], // '/'
        [0x3E,0x51,0x49,0x45,0x3E], // '0'
        [0x00,0x42,0x7F,0x40,0x00], // '1'
        [0x42,0x61,0x51,0x49,0x46], // '2'
        [0x21,0x41,0x45,0x4B,0x31], // '3'
        [0x18,0x14,0x12,0x7F,0x10], // '4'
        [0x27,0x45,0x45,0x45,0x39], // '5'
        [0x3C,0x4A,0x49,0x49,0x30], // '6'
        [0x01,0x71,0x09,0x05,0x03], // '7'
        [0x36,0x49,0x49,0x49,0x36], // '8'
        [0x06,0x49,0x49,0x29,0x1E], // '9'
        [0x00,0x36,0x36,0x00,0x00], // ':'
        [0x00,0x56,0x36,0x00,0x00], // ';'
        [0x08,0x14,0x22,0x41,0x00], // '<'
        [0x14,0x14,0x14,0x14,0x14], // '='
        [0x00,0x41,0x22,0x14,0x08], // '>'
        [0x02,0x01,0x51,0x09,0x06], // '?'
        [0x32,0x49,0x79,0x41,0x3E], // '@'
        [0x7E,0x11,0x11,0x11,0x7E], // 'A'
        [0x7F,0x49,0x49,0x49,0x36], // 'B'
        [0x3E,0x41,0x41,0x41,0x22], // 'C'
        [0x7F,0x41,0x41,0x22,0x1C], // 'D'
        [0x7F,0x49,0x49,0x49,0x41], // 'E'
        [0x7F,0x09,0x09,0x09,0x01], // 'F'
        [0x3E,0x41,0x49,0x49,0x7A], // 'G'
        [0x7F,0x08,0x08,0x08,0x7F], // 'H'
        [0x00,0x41,0x7F,0x41,0x00], // 'I'
        [0x20,0x40,0x41,0x3F,0x01], // 'J'
        [0x7F,0x08,0x14,0x22,0x41], // 'K'
        [0x7F,0x40,0x40,0x40,0x40], // 'L'
        [0x7F,0x02,0x0C,0x02,0x7F], // 'M'
        [0x7F,0x04,0x08,0x10,0x7F], // 'N'
        [0x3E,0x41,0x41,0x41,0x3E], // 'O'
        [0x7F,0x09,0x09,0x09,0x06], // 'P'
        [0x3E,0x41,0x51,0x21,0x5E], // 'Q'
        [0x7F,0x09,0x19,0x29,0x46], // 'R'
        [0x46,0x49,0x49,0x49,0x31], // 'S'
        [0x01,0x01,0x7F,0x01,0x01], // 'T'
        [0x3F,0x40,0x40,0x40,0x3F], // 'U'
        [0x1F,0x20,0x40,0x20,0x1F], // 'V'
        [0x3F,0x40,0x38,0x40,0x3F], // 'W'
        [0x63,0x14,0x08,0x14,0x63], // 'X'
        [0x07,0x08,0x70,0x08,0x07], // 'Y'
        [0x61,0x51,0x49,0x45,0x43], // 'Z'
        [0x00,0x7F,0x41,0x41,0x00], // '['
        [0x02,0x04,0x08,0x10,0x20], // '\'
        [0x00,0x41,0x41,0x7F,0x00], // ']'
        [0x04,0x02,0x01,0x02,0x04], // '^'
        [0x40,0x40,0x40,0x40,0x40], // '_'
        [0x00,0x01,0x02,0x04,0x00], // '`'
        [0x20,0x54,0x54,0x54,0x78], // 'a'
        [0x7F,0x48,0x44,0x44,0x38], // 'b'
        [0x38,0x44,0x44,0x44,0x20], // 'c'
        [0x38,0x44,0x44,0x48,0x7F], // 'd'
        [0x38,0x54,0x54,0x54,0x18], // 'e'
        [0x08,0x7E,0x09,0x01,0x02], // 'f'
        [0x0C,0x52,0x52,0x52,0x3E], // 'g'
        [0x7F,0x08,0x04,0x04,0x78], // 'h'
        [0x00,0x44,0x7D,0x40,0x00], // 'i'
        [0x20,0x40,0x44,0x3D,0x00], // 'j'
        [0x7F,0x10,0x28,0x44,0x00], // 'k'
        [0x00,0x41,0x7F,0x40,0x00], // 'l'
        [0x7C,0x04,0x18,0x04,0x78], // 'm'
        [0x7C,0x08,0x04,0x04,0x78], // 'n'
        [0x38,0x44,0x44,0x44,0x38], // 'o'
        [0x7C,0x14,0x14,0x14,0x08], // 'p'
        [0x08,0x14,0x14,0x18,0x7C], // 'q'
        [0x7C,0x08,0x04,0x04,0x08], // 'r'
        [0x48,0x54,0x54,0x54,0x20], // 's'
        [0x04,0x3F,0x44,0x40,0x20], // 't'
        [0x3C,0x40,0x40,0x20,0x7C], // 'u'
        [0x1C,0x20,0x40,0x20,0x1C], // 'v'
        [0x3C,0x40,0x30,0x40,0x3C], // 'w'
        [0x44,0x28,0x10,0x28,0x44], // 'x'
        [0x0C,0x50,0x50,0x50,0x3C], // 'y'
        [0x44,0x64,0x54,0x4C,0x44], // 'z'
        [0x00,0x08,0x36,0x41,0x00], // '{'
        [0x00,0x00,0x7F,0x00,0x00], // '|'
        [0x00,0x41,0x36,0x08,0x00], // '}'
        [0x10,0x08,0x08,0x10,0x08], // '~'
        [0x78,0x46,0x41,0x46,0x78], // DEL placeholder
    ];

    pub fn draw_text(s: &str, x: f32, y: f32) {
        let fc = match with_style(|s| s.fill) { Some(c) => c, None => return };
        // Each font pixel maps to `cell` output pixels.  Round to the nearest
        // integer so pixels are square and there are no spacing gaps.
        let cell = ((TEXT_SIZE.with(|ts| *ts.borrow()) / 8.0).max(1.0).round()) as i32;
        let cell_f = cell as f32;
        let (cw, canvas_h) = canvas_dims();
        let (tx, ty) = tfm(x, y);
        let mut cursor_x = tx;
        let mut buf = pixel_buf().lock();
        for byte in s.bytes() {
            if byte < 32 || byte > 127 { cursor_x += 6.0 * cell_f; continue; }
            let glyph = &FONT5X8[(byte - 32) as usize];
            for col in 0..5usize {
                let bits = glyph[col];
                for row in 0..8usize {
                    if bits & (1 << row) != 0 {
                        let px = (cursor_x + col as f32 * cell_f) as i32;
                        let py = (ty + row as f32 * cell_f) as i32;
                        for dy in 0..cell {
                            for dx in 0..cell {
                                put(&mut buf, cw, canvas_h, px + dx, py + dy, fc);
                            }
                        }
                    }
                }
            }
            cursor_x += 6.0 * cell_f; // 5 cols + 1 px gap
        }
    }

    /// Width in pixels of `s` at the current text size.
    pub fn text_width(s: &str) -> f32 {
        let cell_f = ((TEXT_SIZE.with(|ts| *ts.borrow()) / 8.0).max(1.0).round()) as f32;
        s.bytes().filter(|b| *b >= 32 && *b <= 127).count() as f32 * 6.0 * cell_f
    }

    pub fn set_text_size(size: f32) { TEXT_SIZE.with(|s| *s.borrow_mut() = size); }

    pub fn set_smooth(_on: bool) {}
}

// ─────────────────────────────────────────────────────────────────────────────
// GLFW / native event loop (minifb-backed)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(not(feature = "wasm"))]
mod glfw {
    use super::{CANVAS_W, CANVAS_H, pixel_buf, resize_canvas};
    use std::sync::atomic::Ordering;
    use std::time::Instant;

    pub enum Event {
        Setup, Draw,
        MousePressed, MouseReleased, MouseMoved, MouseDragged,
        MouseWheel(f32),
        KeyPressed, KeyReleased, KeyTyped,
    }

    #[derive(Clone)]
    pub struct FrameContext {
        pub mouse_x: f32, pub mouse_y: f32,
        pub width: u32,   pub height: u32,
        pub frame_count: u64, pub mouse_pressed: bool,
        pub key: Option<char>, pub key_code: u32, pub key_pressed: bool,
        pub millis: u64,  pub focused: bool,
    }
    impl FrameContext {
        pub fn mouse_x(&self)       -> f32          { self.mouse_x }
        pub fn mouse_y(&self)       -> f32          { self.mouse_y }
        pub fn width(&self)         -> u32          { self.width }
        pub fn height(&self)        -> u32          { self.height }
        pub fn frame_count(&self)   -> u64          { self.frame_count }
        pub fn mouse_pressed(&self) -> bool         { self.mouse_pressed }
        pub fn key(&self)           -> Option<char> { self.key }
        pub fn key_code(&self)      -> u32          { self.key_code }
        pub fn key_pressed(&self)   -> bool         { self.key_pressed }
        pub fn millis(&self)        -> u64          { self.millis }
        pub fn focused(&self)       -> bool         { self.focused }
        fn default(w: u32, h: u32) -> Self {
            Self { mouse_x: 0.0, mouse_y: 0.0, width: w, height: h,
                   frame_count: 0, mouse_pressed: false, key: None,
                   key_code: 0, key_pressed: false, millis: 0, focused: true }
        }
    }

    // Global desired FPS / loop control
    use std::sync::{Mutex, OnceLock};
    static FRAME_RATE: OnceLock<Mutex<f32>>  = OnceLock::new();
    static LOOPING:    OnceLock<Mutex<bool>> = OnceLock::new();
    static WANT_W:     OnceLock<Mutex<u32>>  = OnceLock::new();
    static WANT_H:     OnceLock<Mutex<u32>>  = OnceLock::new();
    static WANT_TITLE: OnceLock<Mutex<String>> = OnceLock::new();

    fn fps() -> f32  { *FRAME_RATE.get_or_init(|| Mutex::new(60.0)).lock().unwrap() }
    fn looping() -> bool { *LOOPING.get_or_init(|| Mutex::new(true)).lock().unwrap() }

    pub fn set_size(w: u32, h: u32) {
        *WANT_W.get_or_init(|| Mutex::new(w)).lock().unwrap() = w;
        *WANT_H.get_or_init(|| Mutex::new(h)).lock().unwrap() = h;
        resize_canvas(w, h);
    }
    pub fn set_fullscreen(_on: bool) {} // not supported on minifb
    pub fn set_frame_rate(f: f32) { *FRAME_RATE.get_or_init(|| Mutex::new(60.0)).lock().unwrap() = f; }
    pub fn set_looping(v: bool)   { *LOOPING.get_or_init(|| Mutex::new(true)).lock().unwrap() = v; }
    pub fn request_redraw()       {}
    pub fn exit_sketch()          { std::process::exit(0); }
    pub fn set_window_title(t: &str) {
        *WANT_TITLE.get_or_init(|| Mutex::new(t.to_owned())).lock().unwrap() = t.to_owned();
    }
    pub fn set_cursor_visible(_v: bool) {}

    pub struct Runner { title: String }
    impl Runner {
        pub fn new() -> Self { Self { title: String::from("sketch") } }
        pub fn title(mut self, t: &str) -> Self { self.title = t.to_owned(); self }

        pub fn start<F>(self, mut cb: F) where F: FnMut(Event, &FrameContext) {
            use minifb::{Window, WindowOptions, Key, MouseMode, MouseButton};

            let init_w = CANVAS_W.load(Ordering::Relaxed);
            let init_h = CANVAS_H.load(Ordering::Relaxed);

            // Run Setup first (may call size() to set dimensions)
            {
                let ctx = FrameContext::default(init_w, init_h);
                cb(Event::Setup, &ctx);
            }

            let w = CANVAS_W.load(Ordering::Relaxed) as usize;
            let h = CANVAS_H.load(Ordering::Relaxed) as usize;

            let title = WANT_TITLE
                .get().map(|m| m.lock().unwrap().clone())
                .unwrap_or_else(|| self.title.clone());

            let mut window = Window::new(
                &title,
                w, h,
                WindowOptions { resize: true, ..WindowOptions::default() },
            ).expect("Failed to create minifb window");

            let target_fps = fps();
            window.set_target_fps(target_fps as usize);

            let start = Instant::now();
            let mut frame_count: u64 = 0;
            let mut last_mouse_pressed = false;
            let mut last_mouse_x = 0.0f32;
            let mut last_mouse_y = 0.0f32;

            while window.is_open() && !window.is_key_down(Key::Escape) {
                let cur_w = window.get_size().0;
                let cur_h = window.get_size().1;
                if cur_w != CANVAS_W.load(Ordering::Relaxed) as usize
                    || cur_h != CANVAS_H.load(Ordering::Relaxed) as usize
                {
                    resize_canvas(cur_w as u32, cur_h as u32);
                }

                let (mx, my) = window.get_mouse_pos(MouseMode::Clamp)
                    .map(|(x,y)| (x, y))
                    .unwrap_or((0.0, 0.0));

                let mouse_pressed = window.get_mouse_down(MouseButton::Left);

                let millis = start.elapsed().as_millis() as u64;
                let mut ctx = FrameContext {
                    mouse_x: mx, mouse_y: my,
                    width:  CANVAS_W.load(Ordering::Relaxed),
                    height: CANVAS_H.load(Ordering::Relaxed),
                    frame_count, mouse_pressed,
                    key: None, key_code: 0, key_pressed: false,
                    millis, focused: true,
                };

                // Mouse events
                if mouse_pressed && !last_mouse_pressed {
                    cb(Event::MousePressed, &ctx);
                } else if !mouse_pressed && last_mouse_pressed {
                    cb(Event::MouseReleased, &ctx);
                }
                if (mx - last_mouse_x).abs() > 0.5 || (my - last_mouse_y).abs() > 0.5 {
                    if mouse_pressed {
                        cb(Event::MouseDragged, &ctx);
                    } else {
                        cb(Event::MouseMoved, &ctx);
                    }
                }

                // Scroll (approximated via key)
                if let Some(scroll) = window.get_scroll_wheel() {
                    cb(Event::MouseWheel(scroll.1), &ctx);
                }

                // Key events (fire once per pressed key)
                let shift = window.is_key_down(Key::LeftShift)
                         || window.is_key_down(Key::RightShift);
                let keys: Vec<Key> = window.get_keys_pressed(minifb::KeyRepeat::No);
                for k in &keys {
                    ctx.key_pressed = true;
                    ctx.key_code = *k as u32;
                    ctx.key = key_to_char(*k, shift);
                    cb(Event::KeyPressed, &ctx);
                    if ctx.key.is_some() { cb(Event::KeyTyped, &ctx); }
                }
                let keys_rel: Vec<Key> = window.get_keys_released();
                for k in &keys_rel {
                    ctx.key_code = *k as u32;
                    ctx.key = key_to_char(*k, shift);
                    cb(Event::KeyReleased, &ctx);
                }

                // Draw frame
                if looping() {
                    // key/key_code stay None/0 so sync_globals won't clobber the
                    // persisted last-pressed values; key_pressed reflects current hold state.
                    ctx.key = None; ctx.key_code = 0;
                    ctx.key_pressed = !window.get_keys().is_empty();
                    ctx.mouse_x = mx; ctx.mouse_y = my; ctx.mouse_pressed = mouse_pressed;
                    cb(Event::Draw, &ctx);
                    frame_count += 1;
                }

                // Blit pixel buffer
                let buf = pixel_buf().lock();
                let cw = CANVAS_W.load(Ordering::Relaxed) as usize;
                let ch = CANVAS_H.load(Ordering::Relaxed) as usize;
                let _ = window.update_with_buffer(&buf, cw, ch);
                drop(buf);

                last_mouse_pressed = mouse_pressed;
                last_mouse_x = mx;
                last_mouse_y = my;
            }
        }
    }

    fn key_to_char(k: minifb::Key, shift: bool) -> Option<char> {
        use minifb::Key::*;
        // Letters: shift → uppercase
        if shift {
            match k {
                A => return Some('A'), B => return Some('B'), C => return Some('C'),
                D => return Some('D'), E => return Some('E'), F => return Some('F'),
                G => return Some('G'), H => return Some('H'), I => return Some('I'),
                J => return Some('J'), K => return Some('K'), L => return Some('L'),
                M => return Some('M'), N => return Some('N'), O => return Some('O'),
                P => return Some('P'), Q => return Some('Q'), R => return Some('R'),
                S => return Some('S'), T => return Some('T'), U => return Some('U'),
                V => return Some('V'), W => return Some('W'), X => return Some('X'),
                Y => return Some('Y'), Z => return Some('Z'),
                // US QWERTY shifted digits/symbols
                Key0 => return Some(')'), Key1 => return Some('!'),
                Key2 => return Some('@'), Key3 => return Some('#'),
                Key4 => return Some('$'), Key5 => return Some('%'),
                Key6 => return Some('^'), Key7 => return Some('&'),
                Key8 => return Some('*'), Key9 => return Some('('),
                Minus => return Some('_'), Equal => return Some('+'),
                LeftBracket => return Some('{'), RightBracket => return Some('}'),
                Semicolon => return Some(':'), Apostrophe => return Some('"'),
                Comma => return Some('<'), Period => return Some('>'),
                Slash => return Some('?'), Backslash => return Some('|'),
                _ => {}
            }
        }
        match k {
            A => Some('a'), B => Some('b'), C => Some('c'), D => Some('d'),
            E => Some('e'), F => Some('f'), G => Some('g'), H => Some('h'),
            I => Some('i'), J => Some('j'), K => Some('k'), L => Some('l'),
            M => Some('m'), N => Some('n'), O => Some('o'), P => Some('p'),
            Q => Some('q'), R => Some('r'), S => Some('s'), T => Some('t'),
            U => Some('u'), V => Some('v'), W => Some('w'), X => Some('x'),
            Y => Some('y'), Z => Some('z'),
            Key0 => Some('0'), Key1 => Some('1'), Key2 => Some('2'),
            Key3 => Some('3'), Key4 => Some('4'), Key5 => Some('5'),
            Key6 => Some('6'), Key7 => Some('7'), Key8 => Some('8'),
            Key9 => Some('9'),
            Minus => Some('-'), Equal => Some('='),
            LeftBracket => Some('['), RightBracket => Some(']'),
            Semicolon => Some(';'), Apostrophe => Some('\''),
            Comma => Some(','), Period => Some('.'), Slash => Some('/'),
            Backslash => Some('\\'), Space => Some(' '), Enter => Some('\n'),
            Tab => Some('\t'), Backspace => Some('\x08'),
            _ => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// WASM stub (no-op; replace with wasm-bindgen + canvas 2D later)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "wasm")]
mod wasm {
    pub enum Event {
        Draw,
        MousePressed, MouseReleased, MouseMoved, MouseDragged,
        MouseWheel(f32),
        KeyPressed, KeyReleased, KeyTyped,
    }

    pub struct FrameContext {
        pub mouse_x: f32, pub mouse_y: f32,
        pub width: u32,   pub height: u32,
        pub frame_count: u64, pub mouse_pressed: bool,
        pub key: Option<char>, pub key_code: u32, pub key_pressed: bool,
        pub millis: u64,  pub focused: bool,
    }
    impl FrameContext {
        pub fn mouse_x(&self)       -> f32          { self.mouse_x }
        pub fn mouse_y(&self)       -> f32          { self.mouse_y }
        pub fn width(&self)         -> u32          { self.width }
        pub fn height(&self)        -> u32          { self.height }
        pub fn frame_count(&self)   -> u64          { self.frame_count }
        pub fn mouse_pressed(&self) -> bool         { self.mouse_pressed }
        pub fn key(&self)           -> Option<char> { self.key }
        pub fn key_code(&self)      -> u32          { self.key_code }
        pub fn key_pressed(&self)   -> bool         { self.key_pressed }
        pub fn millis(&self)        -> u64          { self.millis }
        pub fn focused(&self)       -> bool         { self.focused }
    }

    pub struct Runner { title: String }
    impl Runner {
        pub fn new() -> Self { Self { title: String::from("sketch") } }
        pub fn title(mut self, t: &str) -> Self { self.title = t.to_owned(); self }
        pub fn start<F>(self, _cb: F) where F: FnMut(Event, &FrameContext) + 'static {}
    }

    pub fn set_size(_w: u32, _h: u32) {}
    pub fn set_frame_rate(_fps: f32) {}
    pub fn set_looping(_on: bool) {}
    pub fn request_redraw() {}
    pub fn exit_sketch() {}
    pub fn set_cursor_visible(_v: bool) {}
}

// ─────────────────────────────────────────────────────────────────────────────
// Public re-exports
// ─────────────────────────────────────────────────────────────────────────────

pub use core::{Color, PVector};

// ──────────────────────────── Global sketch state ─────────────────────────────

#[derive(Debug)]
struct Globals {
    mouse_x: f32, mouse_y: f32, pmouse_x: f32, pmouse_y: f32,
    width: f32, height: f32, frame_count: u64,
    mouse_pressed: bool,
    // `key` persists — it holds the last pressed character, exactly like Processing.
    key: char, key_code: u32, key_pressed: bool,
    millis: u64, focused: bool,
}

impl Default for Globals {
    fn default() -> Self {
        Self {
            mouse_x: 0.0, mouse_y: 0.0, pmouse_x: 0.0, pmouse_y: 0.0,
            width: 0.0, height: 0.0, frame_count: 0,
            mouse_pressed: false,
            key: '\0', key_code: 0, key_pressed: false,
            millis: 0, focused: false,
        }
    }
}

static GLOBALS: OnceLock<RwLock<Globals>> = OnceLock::new();
fn globals() -> &'static RwLock<Globals> {
    GLOBALS.get_or_init(|| RwLock::new(Globals::default()))
}

#[cfg(not(feature = "wasm"))]
fn sync_globals(ctx: &glfw::FrameContext) {
    let mut g = globals().write();
    g.pmouse_x = g.mouse_x; g.pmouse_y = g.mouse_y;
    g.mouse_x     = ctx.mouse_x();
    g.mouse_y     = ctx.mouse_y();
    g.width       = ctx.width() as f32;
    g.height      = ctx.height() as f32;
    g.frame_count = ctx.frame_count();
    g.mouse_pressed = ctx.mouse_pressed();
    // `key` and `key_code` persist across frames (Processing semantics): only update
    // when the context carries an actual key event (not the Draw call's cleared fields).
    if let Some(k) = ctx.key() { g.key = k; }
    if ctx.key_code() != 0     { g.key_code = ctx.key_code(); }
    g.key_pressed = ctx.key_pressed();
    g.millis      = ctx.millis();
    g.focused     = ctx.focused();
}

// ─────────────────────────── Public globals API ───────────────────────────────

#[inline] pub fn mouse_x()  -> f32  { globals().read().mouse_x }
#[inline] pub fn mouse_y()  -> f32  { globals().read().mouse_y }
#[inline] pub fn pmouse_x() -> f32  { globals().read().pmouse_x }
#[inline] pub fn pmouse_y() -> f32  { globals().read().pmouse_y }
#[inline] pub fn width()    -> f32  { globals().read().width }
#[inline] pub fn height()   -> f32  { globals().read().height }
#[inline] pub fn frame_count() -> u64  { globals().read().frame_count }
#[inline] pub fn mouse_pressed_global() -> bool { globals().read().mouse_pressed }
/// Last key pressed (persists across frames, '\0' before any key event).
#[inline] pub fn key()      -> char  { globals().read().key }
#[inline] pub fn key_code() -> u32   { globals().read().key_code }
#[inline] pub fn key_pressed_global() -> bool { globals().read().key_pressed }
#[inline] pub fn millis()   -> u64   { globals().read().millis }
#[inline] pub fn focused()  -> bool  { globals().read().focused }

// camelCase aliases — these are what the emitter generates for bare identifier references.
#[inline] pub fn mouseX()      -> f32  { mouse_x() }
#[inline] pub fn mouseY()      -> f32  { mouse_y() }
#[inline] pub fn pmouseX()     -> f32  { pmouse_x() }
#[inline] pub fn pmouseY()     -> f32  { pmouse_y() }
#[inline] pub fn frameCount()  -> u64  { frame_count() }
#[inline] pub fn mousePressed() -> bool { mouse_pressed_global() }
#[inline] pub fn keyPressed()  -> bool { key_pressed_global() }
#[inline] pub fn keyCode()     -> u32  { key_code() }

// ── Mouse wheel delta ─────────────────────────────────────────────────────────

static WHEEL_DELTA: OnceLock<parking_lot::Mutex<f32>> = OnceLock::new();
fn wheel_cell() -> &'static parking_lot::Mutex<f32> {
    WHEEL_DELTA.get_or_init(|| parking_lot::Mutex::new(0.0))
}
#[doc(hidden)]
pub fn __set_wheel_delta(d: f32) { *wheel_cell().lock() = d; }
#[inline] pub fn mouse_wheel_delta() -> f32 { *wheel_cell().lock() }

// ──────────────────────────── Canvas / window ────────────────────────────────

pub fn size(w: u32, h: u32) {
    #[cfg(not(feature = "wasm"))] glfw::set_size(w, h);
    #[cfg(feature = "wasm")]      wasm::set_size(w, h);
}
pub fn full_screen() {
    #[cfg(not(feature = "wasm"))] glfw::set_fullscreen(true);
}
#[inline] pub fn fullScreen() { full_screen() }

pub fn frame_rate(fps: f32) {
    #[cfg(not(feature = "wasm"))] glfw::set_frame_rate(fps);
    #[cfg(feature = "wasm")]      wasm::set_frame_rate(fps);
}
#[inline] pub fn frameRate(fps: f32) { frame_rate(fps) }

// ──────────────────────────── Background / color ─────────────────────────────

pub fn background(gray: f32)                              { render::background_gray(gray / 255.0); }
pub fn background_rgb(r: f32, g: f32, b: f32)            { render::background_rgb(r/255.0, g/255.0, b/255.0); }
pub fn background_rgba(r: f32, g: f32, b: f32, a: f32)   { render::background_rgba(r/255.0, g/255.0, b/255.0, a/255.0); }
pub fn background_color(c: Color)                         { render::background_color(c); }

pub fn fill(gray: f32)                                    { render::set_fill_gray(gray / 255.0); }
pub fn fill_rgb(r: f32, g: f32, b: f32)                  { render::set_fill_rgb(r/255.0, g/255.0, b/255.0); }
pub fn fill_rgba(r: f32, g: f32, b: f32, a: f32)         { render::set_fill_rgba(r/255.0, g/255.0, b/255.0, a/255.0); }
pub fn fill_color(c: Color)                               { render::set_fill_color(c); }
pub fn no_fill()                                          { render::set_fill_none(); }
#[inline] pub fn noFill() { no_fill() }

pub fn stroke(gray: f32)                                  { render::set_stroke_gray(gray / 255.0); }
pub fn stroke_rgb(r: f32, g: f32, b: f32)                { render::set_stroke_rgb(r/255.0, g/255.0, b/255.0); }
pub fn stroke_rgba(r: f32, g: f32, b: f32, a: f32)       { render::set_stroke_rgba(r/255.0, g/255.0, b/255.0, a/255.0); }
pub fn stroke_color(c: Color)                             { render::set_stroke_color(c); }
pub fn no_stroke()                                        { render::set_stroke_none(); }
#[inline] pub fn noStroke() { no_stroke() }

pub fn stroke_weight(w: f32)                              { render::set_stroke_weight(w); }
#[inline] pub fn strokeWeight(w: f32) { stroke_weight(w) }

// ─────────────────────────── Primitive shapes ────────────────────────────────

pub fn ellipse(x: f32, y: f32, w: f32, h: f32)           { render::draw_ellipse(x, y, w, h); }
pub fn circle(x: f32, y: f32, d: f32)                    { render::draw_ellipse(x, y, d, d); }
pub fn rect(x: f32, y: f32, w: f32, h: f32)              { render::draw_rect(x, y, w, h); }
pub fn rect_rounded(x: f32, y: f32, w: f32, h: f32, r: f32) { render::draw_rect_rounded(x, y, w, h, r); }
#[inline] pub fn rectRounded(x: f32, y: f32, w: f32, h: f32, r: f32) { rect_rounded(x, y, w, h, r) }
pub fn square(x: f32, y: f32, s: f32)                    { render::draw_rect(x, y, s, s); }
pub fn triangle(x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) { render::draw_triangle(x1, y1, x2, y2, x3, y3); }
pub fn line(x1: f32, y1: f32, x2: f32, y2: f32)          { render::draw_line(x1, y1, x2, y2); }
pub fn point(x: f32, y: f32)                              { render::draw_point(x, y); }
pub fn quad(x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32, x4: f32, y4: f32) {
    render::draw_quad(x1, y1, x2, y2, x3, y3, x4, y4);
}
pub fn arc(x: f32, y: f32, w: f32, h: f32, start: f32, stop: f32) { render::draw_arc(x, y, w, h, start, stop); }

// ─────────────────────────── Transforms ──────────────────────────────────────

pub fn push_matrix()              { render::push_matrix(); }
pub fn pop_matrix()               { render::pop_matrix(); }
#[inline] pub fn pushMatrix() { push_matrix() }
#[inline] pub fn popMatrix()  { pop_matrix()  }
pub fn translate(x: f32, y: f32) { render::translate(x, y, 0.0); }
pub fn translate_3d(x: f32, y: f32, z: f32) { render::translate(x, y, z); }
pub fn rotate(angle: f32)         { render::rotate_z(angle); }
pub fn scale(s: f32)              { render::scale(s, s, 1.0); }
pub fn scale_xy(sx: f32, sy: f32) { render::scale(sx, sy, 1.0); }

// ──────────────────────────── Style state ────────────────────────────────────

pub fn push_style() { render::push_style(); }
pub fn pop_style()  { render::pop_style(); }
#[inline] pub fn pushStyle() { push_style() }
#[inline] pub fn popStyle()  { pop_style()  }

// ─────────────────────────── Text ────────────────────────────────────────────

pub fn text(s: &str, x: f32, y: f32)  { render::draw_text(s, x, y); }
pub fn text_size(size: f32)            { render::set_text_size(size); }
pub fn text_width(s: &str) -> f32      { render::text_width(s) }
#[inline] pub fn textSize(size: f32)  { text_size(size) }
#[inline] pub fn textWidth(s: &str) -> f32 { text_width(s) }

// ─────────────────────────── Math ────────────────────────────────────────────

pub use std::f32::consts::PI;
pub use std::f32::consts::TAU;
pub const TWO_PI:     f32 = std::f32::consts::TAU;
pub const HALF_PI:    f32 = std::f32::consts::FRAC_PI_2;
pub const QUARTER_PI: f32 = std::f32::consts::FRAC_PI_4;

// ─────────────────────────── Key code constants ───────────────────────────────
// Matches minifb::Key discriminant values returned by key_code().
// Usage:  if key_code() == LEFT { ... }
pub const UP:        u32 = 54;
pub const DOWN:      u32 = 51;
pub const LEFT:      u32 = 52;
pub const RIGHT:     u32 = 53;
pub const ENTER:     u32 = 69;
pub const BACKSPACE: u32 = 66;
pub const DELETE:    u32 = 67;
pub const ESCAPE:    u32 = 70;
pub const TAB:       u32 = 78;
pub const HOME:      u32 = 71;
pub const END:       u32 = 68;
pub const PAGE_UP:   u32 = 75;
pub const PAGE_DOWN: u32 = 74;
pub const SHIFT:     u32 = 82;
pub const CONTROL:   u32 = 84;
pub const F1:        u32 = 36;
pub const F2:        u32 = 37;
pub const F3:        u32 = 38;
pub const F4:        u32 = 39;
pub const F5:        u32 = 40;
pub const F6:        u32 = 41;
pub const F7:        u32 = 42;
pub const F8:        u32 = 43;
pub const F9:        u32 = 44;
pub const F10:       u32 = 45;
pub const F11:       u32 = 46;
pub const F12:       u32 = 47;

#[inline] pub fn radians(deg: f32) -> f32 { deg.to_radians() }
#[inline] pub fn degrees(rad: f32) -> f32 { rad.to_degrees() }
#[inline] pub fn constrain(val: f32, low: f32, high: f32) -> f32 { val.clamp(low, high) }
#[inline] pub fn map(val: f32, start1: f32, stop1: f32, start2: f32, stop2: f32) -> f32 {
    start2 + (stop2 - start2) * ((val - start1) / (stop1 - start1))
}
#[inline] pub fn lerp(start: f32, stop: f32, t: f32) -> f32 { start + (stop - start) * t }
#[inline] pub fn dist(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    ((x2-x1).powi(2) + (y2-y1).powi(2)).sqrt()
}
#[inline] pub fn mag(x: f32, y: f32) -> f32 { (x*x + y*y).sqrt() }
#[inline] pub fn abs(v: f32)  -> f32 { v.abs() }
#[inline] pub fn ceil(v: f32) -> f32 { v.ceil() }
#[inline] pub fn floor(v: f32) -> f32 { v.floor() }
#[inline] pub fn round(v: f32) -> f32 { v.round() }
#[inline] pub fn sqrt(v: f32) -> f32 { v.sqrt() }
#[inline] pub fn pow(base: f32, exp: f32) -> f32 { base.powf(exp) }
#[inline] pub fn log(v: f32)  -> f32 { v.ln() }
#[inline] pub fn exp(v: f32)  -> f32 { v.exp() }
#[inline] pub fn min(a: f32, b: f32) -> f32 { a.min(b) }
#[inline] pub fn max(a: f32, b: f32) -> f32 { a.max(b) }

pub fn noise(x: f32) -> f32 { core::noise1(x) }
pub fn noise2(x: f32, y: f32) -> f32 { core::noise2(x, y) }

// ─────────────────────────── Random ──────────────────────────────────────────

pub fn random(high: f32) -> f32 { core::random_f32(0.0, high) }
pub fn random_range(low: f32, high: f32) -> f32 { core::random_f32(low, high) }
pub fn random_seed(seed: u64) { core::random_seed(seed); }
#[inline] pub fn randomSeed(seed: u64) { random_seed(seed) }

// ─────────────────────────── Color constructors ───────────────────────────────

#[inline] pub fn color(gray: f32) -> Color { Color::from_gray(gray / 255.0) }
#[inline] pub fn color_rgb(r: f32, g: f32, b: f32) -> Color { Color::from_rgb(r/255.0, g/255.0, b/255.0) }
#[inline] pub fn color_rgba(r: f32, g: f32, b: f32, a: f32) -> Color { Color::from_rgba(r/255.0, g/255.0, b/255.0, a/255.0) }
pub fn lerp_color(c1: Color, c2: Color, t: f32) -> Color { core::lerp_color(c1, c2, t) }
#[inline] pub fn lerpColor(c1: Color, c2: Color, t: f32) -> Color { lerp_color(c1, c2, t) }

// ─────────────────────────── Console ─────────────────────────────────────────

/// Print a value to the console with a newline (mirrors Processing's `println()`).
pub fn println(s: impl std::fmt::Display) { ::std::println!("{}", s); }
/// Print a value to the console without a newline (mirrors Processing's `print()`).
pub fn print(s: impl std::fmt::Display)   { ::std::print!("{}", s); }

// ─────────────────────────── Sketch control ──────────────────────────────────

pub fn no_loop() {
    #[cfg(not(feature = "wasm"))] glfw::set_looping(false);
    #[cfg(feature = "wasm")]      wasm::set_looping(false);
}
#[inline] pub fn noLoop() { no_loop() }

pub fn r#loop() {
    #[cfg(not(feature = "wasm"))] glfw::set_looping(true);
    #[cfg(feature = "wasm")]      wasm::set_looping(true);
}

pub fn redraw() {
    #[cfg(not(feature = "wasm"))] glfw::request_redraw();
    #[cfg(feature = "wasm")]      wasm::request_redraw();
}

pub fn exit() {
    #[cfg(not(feature = "wasm"))] glfw::exit_sketch();
    #[cfg(feature = "wasm")]      wasm::exit_sketch();
}

pub fn window_title(t: &str) {
    #[cfg(not(feature = "wasm"))] glfw::set_window_title(t);
}
#[inline] pub fn windowTitle(t: &str) { window_title(t) }

pub fn smooth()    { render::set_smooth(true); }
pub fn no_smooth() { render::set_smooth(false); }
#[inline] pub fn noSmooth() { no_smooth() }

pub fn no_cursor() {
    #[cfg(not(feature = "wasm"))] glfw::set_cursor_visible(false);
    #[cfg(feature = "wasm")]      wasm::set_cursor_visible(false);
}
#[inline] pub fn noCursor() { no_cursor() }

pub fn cursor() {
    #[cfg(not(feature = "wasm"))] glfw::set_cursor_visible(true);
    #[cfg(feature = "wasm")]      wasm::set_cursor_visible(true);
}

// ─────────────────────────────── App builder ─────────────────────────────────

pub struct App<S: 'static> {
    state:             S,
    setup_fn:          fn(&mut S),
    draw_fn:           fn(&mut S),
    title:             String,
    mouse_pressed_fn:  Option<fn(&mut S)>,
    mouse_released_fn: Option<fn(&mut S)>,
    mouse_moved_fn:    Option<fn(&mut S)>,
    mouse_dragged_fn:  Option<fn(&mut S)>,
    mouse_wheel_fn:    Option<fn(&mut S, f32)>,
    key_pressed_fn:    Option<fn(&mut S)>,
    key_released_fn:   Option<fn(&mut S)>,
    key_typed_fn:      Option<fn(&mut S)>,
}

impl<S: 'static> App<S> {
    pub fn new(state: S, setup_fn: fn(&mut S), draw_fn: fn(&mut S)) -> Self {
        Self {
            state, setup_fn, draw_fn,
            title: String::from("sketch"),
            mouse_pressed_fn: None, mouse_released_fn: None,
            mouse_moved_fn: None,   mouse_dragged_fn: None,
            mouse_wheel_fn: None,   key_pressed_fn: None,
            key_released_fn: None,  key_typed_fn: None,
        }
    }

    pub fn title(mut self, t: &str) -> Self { self.title = t.to_owned(); self }

    pub fn mouse_pressed(mut self, f: fn(&mut S)) -> Self  { self.mouse_pressed_fn  = Some(f); self }
    pub fn mouse_released(mut self, f: fn(&mut S)) -> Self { self.mouse_released_fn = Some(f); self }
    pub fn mouse_moved(mut self, f: fn(&mut S)) -> Self    { self.mouse_moved_fn    = Some(f); self }
    pub fn mouse_dragged(mut self, f: fn(&mut S)) -> Self  { self.mouse_dragged_fn  = Some(f); self }
    pub fn mouse_wheel(mut self, f: fn(&mut S, f32)) -> Self { self.mouse_wheel_fn  = Some(f); self }
    pub fn key_pressed(mut self, f: fn(&mut S)) -> Self    { self.key_pressed_fn    = Some(f); self }
    pub fn key_released(mut self, f: fn(&mut S)) -> Self   { self.key_released_fn   = Some(f); self }
    pub fn key_typed(mut self, f: fn(&mut S)) -> Self      { self.key_typed_fn      = Some(f); self }

    pub fn run(mut self) {
        #[cfg(not(feature = "wasm"))]
        {
            let runner = glfw::Runner::new().title(&self.title);
            runner.start(move |event, ctx| {
                use glfw::Event;
                match event {
                    Event::Setup         => { (self.setup_fn)(&mut self.state); }
                    Event::Draw          => {
                        render::begin_draw();
                        sync_globals(ctx);
                        (self.draw_fn)(&mut self.state);
                        render::end_draw();
                        render::present();
                    }
                    // Sync globals before every event callback so mouseX()/mouseY()/key()
                    // reflect the position and key that triggered this specific event.
                    Event::MousePressed  => { sync_globals(ctx); if let Some(f) = self.mouse_pressed_fn  { f(&mut self.state); } }
                    Event::MouseReleased => { sync_globals(ctx); if let Some(f) = self.mouse_released_fn { f(&mut self.state); } }
                    Event::MouseMoved    => { sync_globals(ctx); if let Some(f) = self.mouse_moved_fn    { f(&mut self.state); } }
                    Event::MouseDragged  => { sync_globals(ctx); if let Some(f) = self.mouse_dragged_fn  { f(&mut self.state); } }
                    Event::MouseWheel(d) => {
                        sync_globals(ctx);
                        __set_wheel_delta(d);
                        if let Some(f) = self.mouse_wheel_fn { f(&mut self.state, d); }
                    }
                    Event::KeyPressed    => { sync_globals(ctx); if let Some(f) = self.key_pressed_fn    { f(&mut self.state); } }
                    Event::KeyReleased   => { sync_globals(ctx); if let Some(f) = self.key_released_fn   { f(&mut self.state); } }
                    Event::KeyTyped      => { sync_globals(ctx); if let Some(f) = self.key_typed_fn      { f(&mut self.state); } }
                }
            });
        }

        #[cfg(feature = "wasm")]
        {
            let runner = wasm::Runner::new().title(&self.title);
            (self.setup_fn)(&mut self.state);

            let mut state         = self.state;
            let draw_fn           = self.draw_fn;
            let mouse_pressed_fn  = self.mouse_pressed_fn;
            let mouse_released_fn = self.mouse_released_fn;
            let mouse_moved_fn    = self.mouse_moved_fn;
            let mouse_dragged_fn  = self.mouse_dragged_fn;
            let mouse_wheel_fn    = self.mouse_wheel_fn;
            let key_pressed_fn    = self.key_pressed_fn;
            let key_released_fn   = self.key_released_fn;
            let key_typed_fn      = self.key_typed_fn;

            runner.start(move |event, ctx| {
                use wasm::Event;
                match event {
                    Event::Draw => {
                        render::begin_draw();
                        {
                            let mut g = globals().write();
                            g.pmouse_x = g.mouse_x; g.pmouse_y = g.mouse_y;
                            g.mouse_x  = ctx.mouse_x(); g.mouse_y = ctx.mouse_y();
                            g.width    = ctx.width() as f32; g.height = ctx.height() as f32;
                            g.frame_count = ctx.frame_count();
                            g.mouse_pressed = ctx.mouse_pressed();
                            if let Some(k) = ctx.key() { g.key = k; }
                            if ctx.key_code() != 0 { g.key_code = ctx.key_code(); }
                            g.key_pressed = ctx.key_pressed();
                            g.millis   = ctx.millis(); g.focused = ctx.focused();
                        }
                        draw_fn(&mut state);
                        render::end_draw();
                        render::present();
                    }
                    Event::MousePressed  => { if let Some(f) = mouse_pressed_fn  { f(&mut state); } }
                    Event::MouseReleased => { if let Some(f) = mouse_released_fn { f(&mut state); } }
                    Event::MouseMoved    => { if let Some(f) = mouse_moved_fn    { f(&mut state); } }
                    Event::MouseDragged  => { if let Some(f) = mouse_dragged_fn  { f(&mut state); } }
                    Event::MouseWheel(d) => { if let Some(f) = mouse_wheel_fn    { f(&mut state, d); } }
                    Event::KeyPressed    => { if let Some(f) = key_pressed_fn    { f(&mut state); } }
                    Event::KeyReleased   => { if let Some(f) = key_released_fn   { f(&mut state); } }
                    Event::KeyTyped      => { if let Some(f) = key_typed_fn      { f(&mut state); } }
                }
            });
        }
    }
}

// Gray + alpha, used by OverloadRewriter for fill(gray, alpha) etc.
pub fn fill_ga(gray: f32, alpha: f32)       { fill_rgba(gray, gray, gray, alpha); }
pub fn stroke_ga(gray: f32, alpha: f32)     { stroke_rgba(gray, gray, gray, alpha); }
pub fn background_ga(gray: f32, alpha: f32) { background_rgba(gray, gray, gray, alpha); }

// ── Shapes ───────────────────────────────────────────────────────────────────
pub const POINTS: u8 = 1;
pub const LINES: u8 = 2;
pub const TRIANGLES: u8 = 3;
pub const TRIANGLE_STRIP: u8 = 4;
pub const TRIANGLE_FAN: u8 = 5;
pub const QUADS: u8 = 6;
pub const QUAD_STRIP: u8 = 7;
pub const CLOSE: u8 = 1;

pub fn begin_shape()                  { render::shape_begin(0); }
pub fn begin_shape_kind(kind: u8)     { render::shape_begin(kind); }
pub fn vertex(x: f32, y: f32)         { render::shape_vertex(x, y); }
pub fn bezier_vertex(cx1: f32, cy1: f32, cx2: f32, cy2: f32, x: f32, y: f32) {
    render::shape_bezier_vertex(cx1, cy1, cx2, cy2, x, y);
}
pub fn quadratic_vertex(cx: f32, cy: f32, x: f32, y: f32) { render::shape_quadratic_vertex(cx, cy, x, y); }
pub fn curve_vertex(x: f32, y: f32)   { render::shape_curve_vertex(x, y); }
pub fn end_shape()                    { render::shape_end(false); }
pub fn end_shape_mode(mode: u8)       { render::shape_end(mode == CLOSE); }

/// Gaussian random number, mean 0 and standard deviation 1 (Box-Muller).
pub fn random_gaussian() -> f32 {
    let u1 = random_range(f32::EPSILON, 1.0);
    let u2 = random(1.0);
    (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos()
}
