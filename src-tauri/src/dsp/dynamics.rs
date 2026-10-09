use super::{coef, db_to_lin, lin_to_db, pf, Filter, Params, SR};

/// Peak follower with instant attack and a short decay, used as the level detector.
struct Detector {
    env: f32,
    decay: f32,
}

impl Detector {
    fn new() -> Self {
        Self { env: 0.0, decay: coef(5.0) }
    }

    fn run(&mut self, x: f32) -> f32 {
        self.env = x.abs().max(self.env * self.decay);
        lin_to_db(self.env)
    }
}

pub struct Gate {
    det: Detector,
    open: f32,
    close: f32,
    att: f32,
    rel: f32,
    hold: usize,
    is_open: bool,
    cnt: usize,
    g: f32,
}

impl Gate {
    pub fn new() -> Self {
        Self { det: Detector::new(), open: 0.0, close: 0.0, att: 0.0, rel: 0.0, hold: 0, is_open: false, cnt: 0, g: 0.0 }
    }
}

impl Filter for Gate {
    fn update(&mut self, p: &Params) {
        self.open = pf(p, "open_threshold", -26.0);
        self.close = pf(p, "close_threshold", -32.0);
        self.att = 1.0 - coef(pf(p, "attack_time", 25.0));
        self.rel = 1.0 - coef(pf(p, "release_time", 150.0));
        self.hold = (pf(p, "hold_time", 200.0) * 0.001 * SR) as usize;
    }

    fn process(&mut self, buf: &mut [f32]) {
        for s in buf {
            let lvl = self.det.run(*s);
            if lvl >= self.open {
                self.is_open = true;
                self.cnt = self.hold;
            } else if self.is_open {
                if lvl >= self.close {
                    self.cnt = self.hold;
                } else if self.cnt > 0 {
                    self.cnt -= 1;
                } else {
                    self.is_open = false;
                }
            }
            let target = if self.is_open { 1.0 } else { 0.0 };
            let k = if target > self.g { self.att } else { self.rel };
            self.g += (target - self.g) * k;
            *s *= self.g;
        }
    }
}

pub struct Compressor {
    det: Detector,
    ratio: f32,
    thr: f32,
    att: f32,
    rel: f32,
    out: f32,
    gr: f32,
}

impl Compressor {
    pub fn new() -> Self {
        Self { det: Detector::new(), ratio: 10.0, thr: -18.0, att: 0.0, rel: 0.0, out: 0.0, gr: 0.0 }
    }
}

impl Filter for Compressor {
    fn update(&mut self, p: &Params) {
        self.ratio = pf(p, "ratio", 10.0).max(1.0);
        self.thr = pf(p, "threshold", -18.0);
        self.att = coef(pf(p, "attack_time", 6.0));
        self.rel = coef(pf(p, "release_time", 60.0));
        self.out = pf(p, "output_gain", 0.0);
    }

    fn process(&mut self, buf: &mut [f32]) {
        for s in buf {
            let over = self.det.run(*s) - self.thr;
            let target = if over > 0.0 { over * (1.0 - 1.0 / self.ratio) } else { 0.0 };
            let c = if target > self.gr { self.att } else { self.rel };
            self.gr = target + c * (self.gr - target);
            *s *= db_to_lin(self.out - self.gr);
        }
    }
}

/// Boosts signal below the threshold, leaves loud signal alone.
pub struct Upward {
    det: Detector,
    ratio: f32,
    thr: f32,
    att: f32,
    rel: f32,
    out: f32,
    boost: f32,
}

impl Upward {
    pub fn new() -> Self {
        Self { det: Detector::new(), ratio: 2.0, thr: -32.0, att: 0.0, rel: 0.0, out: 0.0, boost: 0.0 }
    }
}

impl Filter for Upward {
    fn update(&mut self, p: &Params) {
        self.ratio = pf(p, "ratio", 2.0).max(1.0);
        self.thr = pf(p, "threshold", -32.0);
        self.att = coef(pf(p, "attack_time", 10.0));
        self.rel = coef(pf(p, "release_time", 100.0));
        self.out = pf(p, "output_gain", 0.0);
    }

    fn process(&mut self, buf: &mut [f32]) {
        for s in buf {
            let under = self.thr - self.det.run(*s);
            let target = if under > 0.0 { (under * (1.0 - 1.0 / self.ratio)).min(30.0) } else { 0.0 };
            let c = if target < self.boost { self.att } else { self.rel };
            self.boost = target + c * (self.boost - target);
            *s *= db_to_lin(self.out + self.boost);
        }
    }
}

/// Attenuates signal below the threshold.
pub struct Expander {
    det: Detector,
    ratio: f32,
    thr: f32,
    att: f32,
    rel: f32,
    out: f32,
    atten: f32,
}

impl Expander {
    pub fn new() -> Self {
        Self { det: Detector::new(), ratio: 4.0, thr: -40.0, att: 0.0, rel: 0.0, out: 0.0, atten: 0.0 }
    }
}

impl Filter for Expander {
    fn update(&mut self, p: &Params) {
        self.ratio = pf(p, "ratio", 4.0).max(1.0);
        self.thr = pf(p, "threshold", -40.0);
        self.att = coef(pf(p, "attack_time", 10.0));
        self.rel = coef(pf(p, "release_time", 50.0));
        self.out = pf(p, "output_gain", 0.0);
    }

    fn process(&mut self, buf: &mut [f32]) {
        for s in buf {
            let under = self.thr - self.det.run(*s);
            let target = if under > 0.0 { (under * (self.ratio - 1.0)).min(60.0) } else { 0.0 };
            let c = if target < self.atten { self.att } else { self.rel };
            self.atten = target + c * (self.atten - target);
            *s *= db_to_lin(self.out - self.atten);
        }
    }
}

pub struct Limiter {
    thr: f32,
    rel: f32,
    g: f32,
}

impl Limiter {
    pub fn new() -> Self {
        Self { thr: db_to_lin(-6.0), rel: 0.0, g: 1.0 }
    }
}

impl Filter for Limiter {
    fn update(&mut self, p: &Params) {
        self.thr = db_to_lin(pf(p, "threshold", -6.0));
        self.rel = coef(pf(p, "release_time", 60.0));
    }

    fn process(&mut self, buf: &mut [f32]) {
        for s in buf {
            let a = s.abs();
            let need = if a > self.thr { self.thr / a } else { 1.0 };
            self.g = (1.0 - (1.0 - self.g) * self.rel).min(need);
            *s *= self.g;
        }
    }
}
