use super::{db_to_lin, pf, Filter, Params, SR};
use std::f32::consts::PI;

pub struct Gain {
    target: f32,
    cur: f32,
}

impl Gain {
    pub fn new() -> Self {
        Self { target: 1.0, cur: 1.0 }
    }
}

impl Filter for Gain {
    fn update(&mut self, p: &Params) {
        self.target = db_to_lin(pf(p, "gain", 0.0));
    }

    fn process(&mut self, buf: &mut [f32]) {
        for s in buf {
            self.cur += (self.target - self.cur) * 0.002;
            *s *= self.cur;
        }
    }
}

pub struct Polarity;

impl Filter for Polarity {
    fn update(&mut self, _: &Params) {}

    fn process(&mut self, buf: &mut [f32]) {
        for s in buf {
            *s = -*s;
        }
    }
}

pub struct Delay {
    buf: Vec<f32>,
    pos: usize,
    delay: usize,
}

impl Delay {
    pub fn new() -> Self {
        Self { buf: vec![0.0; 2 * SR as usize], pos: 0, delay: 0 }
    }
}

impl Filter for Delay {
    fn update(&mut self, p: &Params) {
        let ms = pf(p, "delay", 0.0).clamp(0.0, 1900.0);
        self.delay = (ms * 0.001 * SR) as usize;
    }

    fn process(&mut self, buf: &mut [f32]) {
        let len = self.buf.len();
        for s in buf {
            self.buf[self.pos] = *s;
            *s = self.buf[(self.pos + len - self.delay) % len];
            self.pos = (self.pos + 1) % len;
        }
    }
}

#[derive(Default, Clone, Copy)]
struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    fn set(&mut self, b: [f32; 3], a: [f32; 3]) {
        self.b0 = b[0] / a[0];
        self.b1 = b[1] / a[0];
        self.b2 = b[2] / a[0];
        self.a1 = a[1] / a[0];
        self.a2 = a[2] / a[0];
    }

    // RBJ cookbook shelving / peaking filters.
    fn low_shelf(&mut self, f0: f32, db: f32, q: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = 2.0 * PI * f0 / SR;
        let (c, al) = (w.cos(), w.sin() / (2.0 * q));
        let t = 2.0 * a.sqrt() * al;
        self.set(
            [a * ((a + 1.0) - (a - 1.0) * c + t), 2.0 * a * ((a - 1.0) - (a + 1.0) * c), a * ((a + 1.0) - (a - 1.0) * c - t)],
            [(a + 1.0) + (a - 1.0) * c + t, -2.0 * ((a - 1.0) + (a + 1.0) * c), (a + 1.0) + (a - 1.0) * c - t],
        );
    }

    fn high_shelf(&mut self, f0: f32, db: f32, q: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = 2.0 * PI * f0 / SR;
        let (c, al) = (w.cos(), w.sin() / (2.0 * q));
        let t = 2.0 * a.sqrt() * al;
        self.set(
            [a * ((a + 1.0) + (a - 1.0) * c + t), -2.0 * a * ((a - 1.0) + (a + 1.0) * c), a * ((a + 1.0) + (a - 1.0) * c - t)],
            [(a + 1.0) - (a - 1.0) * c + t, 2.0 * ((a - 1.0) - (a + 1.0) * c), (a + 1.0) - (a - 1.0) * c - t],
        );
    }

    fn peaking(&mut self, f0: f32, db: f32, q: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = 2.0 * PI * f0 / SR;
        let (c, al) = (w.cos(), w.sin() / (2.0 * q));
        self.set([1.0 + al * a, -2.0 * c, 1.0 - al * a], [1.0 + al / a, -2.0 * c, 1.0 - al / a]);
    }

    fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

pub struct Eq3 {
    low: Biquad,
    mid: Biquad,
    high: Biquad,
}

impl Eq3 {
    pub fn new() -> Self {
        let mut e = Self { low: Biquad::default(), mid: Biquad::default(), high: Biquad::default() };
        e.update(&Params::new());
        e
    }
}

impl Filter for Eq3 {
    fn update(&mut self, p: &Params) {
        self.low.low_shelf(800.0, pf(p, "low", 0.0), 0.707);
        self.mid.peaking(2000.0, pf(p, "mid", 0.0), 0.5);
        self.high.high_shelf(5000.0, pf(p, "high", 0.0), 0.707);
    }

    fn process(&mut self, buf: &mut [f32]) {
        for s in buf {
            *s = self.high.run(self.mid.run(self.low.run(*s)));
        }
    }
}
