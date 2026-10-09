use super::nvidia::Nvidia;
use super::{pf, ps, Filter, Params, FRAME};
use nnnoiseless::DenoiseState;

pub struct Denoise {
    rnnoise: Box<DenoiseState<'static>>,
    primed: bool,
    nvidia: Option<Nvidia>,
    nvidia_failed: bool,
    want_nvidia: bool,
    intensity: f32,
    dry: [f32; FRAME],
    wet: [f32; FRAME],
}

impl Denoise {
    pub fn new() -> Self {
        Self {
            rnnoise: DenoiseState::new(),
            primed: false,
            nvidia: None,
            nvidia_failed: false,
            want_nvidia: false,
            intensity: 1.0,
            dry: [0.0; FRAME],
            wet: [0.0; FRAME],
        }
    }
}

impl Filter for Denoise {
    fn update(&mut self, p: &Params) {
        self.want_nvidia = ps(p, "method", "rnnoise") == "nvidia";
        self.intensity = pf(p, "intensity", 1.0).clamp(0.0, 1.0);
        if self.want_nvidia && self.nvidia.is_none() && !self.nvidia_failed {
            match Nvidia::open(self.intensity) {
                Ok(n) => self.nvidia = Some(n),
                Err(e) => {
                    eprintln!("NVIDIA denoiser unavailable, using RNNoise: {e}");
                    self.nvidia_failed = true;
                }
            }
        }
        if !self.want_nvidia {
            self.nvidia_failed = false;
        }
        if let Some(n) = self.nvidia.as_mut() {
            n.set_intensity(self.intensity);
        }
    }

    fn process(&mut self, buf: &mut [f32]) {
        for frame in buf.chunks_exact_mut(FRAME) {
            self.dry.copy_from_slice(frame);

            let used_nvidia = match (self.want_nvidia, self.nvidia.as_mut()) {
                (true, Some(n)) => n.process(&self.dry, frame),
                _ => false,
            };
            if used_nvidia {
                continue;
            }

            // RNNoise works on 16-bit-scaled floats.
            for (d, s) in self.dry.iter_mut().zip(frame.iter()) {
                *d = *s * 32768.0;
            }
            self.rnnoise.process_frame(&mut self.wet, &self.dry);
            let mix = self.intensity;
            for ((o, w), d) in frame.iter_mut().zip(self.wet.iter()).zip(self.dry.iter()) {
                *o = (w * mix + d * (1.0 - mix)) / 32768.0;
            }
            // The first RNNoise frame is a fade-in artifact.
            if !self.primed {
                self.primed = true;
                frame.fill(0.0);
            }
        }
    }
}
