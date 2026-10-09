mod basic;
mod denoise;
mod dynamics;
mod nvidia;

use serde::Deserialize;

pub use nvidia::{probe as nvidia_probe, Status as NvidiaStatus};

pub const SR: f32 = 48000.0;
pub const FRAME: usize = 480;

pub type Params = serde_json::Map<String, serde_json::Value>;

pub fn pf(p: &Params, k: &str, d: f32) -> f32 {
    p.get(k).and_then(|v| v.as_f64()).map(|v| v as f32).unwrap_or(d)
}

pub fn ps<'a>(p: &'a Params, k: &str, d: &'a str) -> &'a str {
    p.get(k).and_then(|v| v.as_str()).unwrap_or(d)
}

pub fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn lin_to_db(x: f32) -> f32 {
    20.0 * x.max(1e-6).log10()
}

/// One-pole smoothing coefficient for a time constant in milliseconds.
pub fn coef(ms: f32) -> f32 {
    if ms <= 0.0 {
        0.0
    } else {
        (-1.0 / (ms * 0.001 * SR)).exp()
    }
}

pub trait Filter: Send {
    fn update(&mut self, p: &Params);
    /// `buf` is always exactly `FRAME` mono samples at 48 kHz.
    fn process(&mut self, buf: &mut [f32]);
}

#[derive(Deserialize, Clone)]
pub struct FilterCfg {
    pub id: String,
    pub kind: String,
    pub enabled: bool,
    #[serde(default)]
    pub params: Params,
}

struct Slot {
    id: String,
    kind: String,
    enabled: bool,
    filter: Box<dyn Filter>,
}

#[derive(Default)]
pub struct Chain {
    slots: Vec<Slot>,
}

fn make(kind: &str) -> Option<Box<dyn Filter>> {
    Some(match kind {
        "denoise" => Box::new(denoise::Denoise::new()),
        "gate" => Box::new(dynamics::Gate::new()),
        "compressor" => Box::new(dynamics::Compressor::new()),
        "upward" => Box::new(dynamics::Upward::new()),
        "expander" => Box::new(dynamics::Expander::new()),
        "limiter" => Box::new(dynamics::Limiter::new()),
        "eq3" => Box::new(basic::Eq3::new()),
        "gain" => Box::new(basic::Gain::new()),
        "polarity" => Box::new(basic::Polarity),
        "delay" => Box::new(basic::Delay::new()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(kind: &str, params: serde_json::Value, input: impl Fn(usize) -> f32, frames: usize) -> Vec<f32> {
        let mut chain = Chain::default();
        let cfg = serde_json::json!([{ "id": "a", "kind": kind, "enabled": true, "params": params }]);
        chain.apply(serde_json::from_value(cfg).unwrap());
        let mut out = Vec::new();
        for f in 0..frames {
            let mut buf: Vec<f32> = (0..FRAME).map(|i| input(f * FRAME + i)).collect();
            chain.process(&mut buf);
            out.extend(buf);
        }
        out
    }

    fn sine(amp: f32) -> impl Fn(usize) -> f32 {
        move |n| amp * (2.0 * std::f32::consts::PI * 220.0 * n as f32 / SR).sin()
    }

    fn peak(v: &[f32]) -> f32 {
        v.iter().fold(0.0, |a, b| a.max(b.abs()))
    }

    #[test]
    fn every_filter_is_finite() {
        for k in ["denoise", "gate", "eq3", "expander", "compressor", "upward", "limiter", "gain", "polarity", "delay"] {
            let out = run(k, serde_json::json!({}), sine(0.5), 20);
            assert!(out.iter().all(|s| s.is_finite()), "{k}");
        }
    }

    #[test]
    fn limiter_caps_peaks() {
        let out = run("limiter", serde_json::json!({ "threshold": -6.0 }), sine(1.0), 10);
        assert!(peak(&out) <= db_to_lin(-6.0) + 1e-4);
    }

    #[test]
    fn gate_closes_on_quiet_and_opens_on_loud() {
        let quiet = run("gate", serde_json::json!({}), sine(0.001), 40);
        assert!(peak(&quiet[quiet.len() - FRAME..]) < 1e-4);
        let loud = run("gate", serde_json::json!({}), sine(0.5), 40);
        assert!(peak(&loud[loud.len() - FRAME..]) > 0.45);
    }

    #[test]
    fn compressor_reduces_loud_signal() {
        let out = run("compressor", serde_json::json!({ "threshold": -30.0, "ratio": 8.0 }), sine(0.5), 40);
        assert!(peak(&out[out.len() - FRAME..]) < 0.3);
    }

    #[test]
    fn eq_flat_is_transparent_and_delay_shifts() {
        let out = run("eq3", serde_json::json!({}), sine(0.5), 20);
        assert!((peak(&out[out.len() - FRAME..]) - 0.5).abs() < 0.01);
        let d = run("delay", serde_json::json!({ "delay": 10.0 }), |n| if n == 0 { 1.0 } else { 0.0 }, 2);
        assert_eq!(d[480], 1.0);
    }

    #[test]
    fn rnnoise_attenuates_steady_noise() {
        let mut seed = 12345u32;
        let noise = move |_| {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ((seed >> 16) as f32 / 32768.0 - 1.0) * 0.002
        };
        let mut n = noise;
        let input: Vec<f32> = (0..FRAME * 100).map(|i| n(i)).collect();
        let out = run("denoise", serde_json::json!({}), |i| input[i], 100);
        let (o, i) = (peak(&out[out.len() - FRAME * 10..]), peak(&input));
        assert!(o < i * 0.5, "out {o} in {i}");
    }
}

impl Chain {
    /// Rebuilds the chain from `cfg`, keeping the state of filters whose id and kind are unchanged.
    pub fn apply(&mut self, cfg: Vec<FilterCfg>) {
        let mut old = std::mem::take(&mut self.slots);
        for c in cfg {
            let mut slot = match old.iter().position(|s| s.id == c.id && s.kind == c.kind) {
                Some(i) => old.swap_remove(i),
                None => match make(&c.kind) {
                    Some(filter) => Slot { id: c.id, kind: c.kind, enabled: c.enabled, filter },
                    None => continue,
                },
            };
            slot.enabled = c.enabled;
            slot.filter.update(&c.params);
            self.slots.push(slot);
        }
    }

    pub fn process(&mut self, buf: &mut [f32]) {
        for s in self.slots.iter_mut().filter(|s| s.enabled) {
            s.filter.process(buf);
        }
    }
}
