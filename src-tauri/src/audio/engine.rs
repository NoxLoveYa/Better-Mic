//! Mic capture -> DSP chain -> virtual cable (and optionally the default speakers for preview).
//! cpal streams are `!Send`, so everything lives on one dedicated thread; the audio callbacks only
//! move samples through lock-free ring buffers.

use super::resample::Resampler;
use super::{default_output_id, find_device};
use crate::dsp::{lin_to_db, Chain, FilterCfg, FRAME, SR};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle, Thread};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const TARGET_BUFFER_MS: f32 = 20.0;

pub enum Msg {
    Chain(Vec<FilterCfg>),
    Preview(bool, mpsc::SyncSender<Result<(), String>>),
    Stop,
}

pub struct EngineHandle {
    tx: mpsc::Sender<Msg>,
    thread: Thread,
    join: Option<JoinHandle<()>>,
}

impl EngineHandle {
    pub fn send(&self, m: Msg) {
        let _ = self.tx.send(m);
        self.thread.unpark();
    }

    /// Starts or stops playing the processed signal on the Windows default output.
    pub fn set_preview(&self, on: bool) -> Result<(), String> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.send(Msg::Preview(on, tx));
        rx.recv_timeout(Duration::from_secs(3)).map_err(|_| "audio thread not responding".to_string())?
    }

    pub fn stop(mut self) {
        self.send(Msg::Stop);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

#[derive(Serialize, Clone, Copy)]
struct MeterPayload {
    in_peak: f32,
    in_rms: f32,
    out_peak: f32,
    out_rms: f32,
    buffered_ms: f32,
}

/// An output device fed with 48 kHz frames. Resamples to the device rate and trims the rate slightly
/// to hold the buffer at its target fill despite clock drift.
struct Sink {
    stream: Stream,
    prod: Producer<f32>,
    cap: usize,
    rate: u32,
    rs: Resampler,
    base_step: f64,
    target_fill: f32,
    fill_avg: f32,
    id: Option<String>,
}

impl Sink {
    fn open(app: &AppHandle, id: Option<&str>, err_event: &'static str) -> Result<Self, String> {
        let dev = find_device(id, false)?;
        let sup = dev.default_output_config().map_err(|e| e.to_string())?;
        let rate = sup.sample_rate();
        let cap = rate as usize / 2;
        let (mut prod, cons) = RingBuffer::<f32>::new(cap);
        let err = err_cb(app, err_event);
        let stream = match sup.sample_format() {
            SampleFormat::F32 => build_output::<f32>(&dev, sup.clone().into(), cons, err),
            SampleFormat::I16 => build_output::<i16>(&dev, sup.clone().into(), cons, err),
            SampleFormat::I32 => build_output::<i32>(&dev, sup.clone().into(), cons, err),
            f => Err(format!("unsupported output sample format {f}")),
        }?;

        let target_fill = rate as f32 * TARGET_BUFFER_MS / 1000.0;
        // Pre-fill with silence so the output callback doesn't underrun while the first frames are processed.
        for _ in 0..target_fill as usize {
            let _ = prod.push(0.0);
        }
        let rs = Resampler::new(SR as f64, rate as f64);
        let base_step = rs.step;
        let id = dev.id().ok().map(|i| i.to_string());
        Ok(Self { stream, prod, cap, rate, rs, base_step, target_fill, fill_avg: target_fill, id })
    }

    fn play(&self) -> Result<(), String> {
        self.stream.play().map_err(|e| e.to_string())
    }

    fn push(&mut self, frame: &[f32]) {
        let fill = (self.cap - self.prod.slots()) as f32;
        self.fill_avg += (fill - self.fill_avg) * 0.02;
        let err = ((self.fill_avg - self.target_fill) / self.target_fill).clamp(-1.0, 1.0) as f64;
        self.rs.step = self.base_step * (1.0 + 0.005 * err);
        let prod = &mut self.prod;
        self.rs.push(frame, |s| {
            let _ = prod.push(s);
        });
    }

    fn buffered_ms(&self) -> f32 {
        self.fill_avg / self.rate as f32 * 1000.0 + 1000.0 * FRAME as f32 / SR
    }
}

struct Io {
    _input: Stream,
    in_cons: Consumer<f32>,
    in_rate: u32,
    out: Sink,
}

pub fn start(
    app: AppHandle,
    input: Option<String>,
    output: Option<String>,
    chain: Vec<FilterCfg>,
    mute: Arc<AtomicBool>,
) -> Result<EngineHandle, String> {
    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::sync_channel::<Result<(), String>>(1);
    let join = thread::spawn(move || match build(&app, input.as_deref(), output.as_deref()) {
        Ok(io) => {
            let _ = ready_tx.send(Ok(()));
            run(app, io, chain, rx, mute);
        }
        Err(e) => {
            let _ = ready_tx.send(Err(e));
        }
    });
    ready_rx.recv().map_err(|_| "audio thread died".to_string())??;
    Ok(EngineHandle { tx, thread: join.thread().clone(), join: Some(join) })
}

fn build(app: &AppHandle, input: Option<&str>, output: Option<&str>) -> Result<Io, String> {
    let in_dev = find_device(input, true)?;
    let in_sup = in_dev.default_input_config().map_err(|e| e.to_string())?;
    let in_rate = in_sup.sample_rate();
    let (in_prod, in_cons) = RingBuffer::<f32>::new(in_rate as usize / 2);

    let wake = thread::current();
    let err_in = err_cb(app, "engine-error");
    let input_stream = match in_sup.sample_format() {
        SampleFormat::F32 => build_input::<f32>(&in_dev, in_sup.clone().into(), in_prod, wake, err_in),
        SampleFormat::I16 => build_input::<i16>(&in_dev, in_sup.clone().into(), in_prod, wake, err_in),
        SampleFormat::I32 => build_input::<i32>(&in_dev, in_sup.clone().into(), in_prod, wake, err_in),
        f => Err(format!("unsupported input sample format {f}")),
    }?;
    let out = Sink::open(app, output, "engine-error")?;

    out.play()?;
    input_stream.play().map_err(|e| e.to_string())?;
    Ok(Io { _input: input_stream, in_cons, in_rate, out })
}

fn open_preview(app: &AppHandle, cable_id: Option<&str>) -> Result<Sink, String> {
    let default = default_output_id();
    if default.is_some() && default.as_deref() == cable_id {
        return Err("The Windows default output is the virtual cable itself. Set your speakers or headphones as the default output to preview.".into());
    }
    let sink = Sink::open(app, None, "preview-error")?;
    sink.play()?;
    Ok(sink)
}

fn err_cb(app: &AppHandle, event: &'static str) -> impl FnMut(cpal::Error) + Send + 'static {
    let app = app.clone();
    move |e| {
        // An xrun is a one-off glitch, not a dead stream, so don't tear anything down for it.
        if e.kind() != cpal::ErrorKind::Xrun {
            let _ = app.emit(event, e.to_string());
        }
    }
}

fn build_input<T>(
    dev: &Device,
    cfg: StreamConfig,
    mut prod: Producer<f32>,
    wake: Thread,
    err: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let ch = cfg.channels as usize;
    dev.build_input_stream(
        cfg,
        move |data: &[T], _| {
            for frame in data.chunks(ch) {
                let mono = frame.iter().map(|s| s.to_sample::<f32>()).sum::<f32>() / ch as f32;
                let _ = prod.push(mono);
            }
            wake.unpark();
        },
        err,
        None,
    )
    .map_err(|e| e.to_string())
}

fn build_output<T>(
    dev: &Device,
    cfg: StreamConfig,
    mut cons: Consumer<f32>,
    err: impl FnMut(cpal::Error) + Send + 'static,
) -> Result<Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let ch = cfg.channels as usize;
    dev.build_output_stream(
        cfg,
        move |data: &mut [T], _| {
            for frame in data.chunks_mut(ch) {
                frame.fill(T::from_sample(cons.pop().unwrap_or(0.0)));
            }
        },
        err,
        None,
    )
    .map_err(|e| e.to_string())
}

#[derive(Default)]
struct Level {
    peak: f32,
    sq: f32,
    n: usize,
}

impl Level {
    fn add(&mut self, buf: &[f32]) {
        for s in buf {
            self.peak = self.peak.max(s.abs());
            self.sq += s * s;
        }
        self.n += buf.len();
    }

    fn take(&mut self) -> (f32, f32) {
        let r = (lin_to_db(self.peak), lin_to_db((self.sq / self.n.max(1) as f32).sqrt()));
        *self = Self::default();
        r
    }
}

fn run(app: AppHandle, mut io: Io, cfg: Vec<FilterCfg>, rx: mpsc::Receiver<Msg>, mute: Arc<AtomicBool>) {
    let mut chain = Chain::default();
    chain.apply(cfg);

    let mut in_rs = (io.in_rate != SR as u32).then(|| Resampler::new(io.in_rate as f64, SR as f64));
    let mut preview: Option<Sink> = None;

    let (mut raw, mut acc) = (Vec::<f32>::new(), Vec::<f32>::new());
    let mut frame = [0f32; FRAME];
    let (mut lin, mut lout) = (Level::default(), Level::default());
    let mut last_emit = Instant::now();

    loop {
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Chain(c) => chain.apply(c),
                Msg::Preview(on, reply) => {
                    preview = None;
                    let res = if on {
                        open_preview(&app, io.out.id.as_deref()).map(|s| {
                            preview = Some(s);
                        })
                    } else {
                        Ok(())
                    };
                    let _ = reply.send(res);
                }
                Msg::Stop => return,
            }
        }

        raw.clear();
        while let Ok(s) = io.in_cons.pop() {
            raw.push(s);
        }
        match in_rs.as_mut() {
            Some(r) => r.push(&raw, |s| acc.push(s)),
            None => acc.extend_from_slice(&raw),
        }

        let mut off = 0;
        while acc.len() - off >= FRAME {
            frame.copy_from_slice(&acc[off..off + FRAME]);
            off += FRAME;

            lin.add(&frame);
            chain.process(&mut frame);
            if mute.load(Ordering::Relaxed) {
                frame.fill(0.0);
            }
            lout.add(&frame);

            io.out.push(&frame);
            if let Some(p) = preview.as_mut() {
                p.push(&frame);
            }
        }
        acc.drain(..off);

        if last_emit.elapsed() >= Duration::from_millis(33) {
            last_emit = Instant::now();
            let (in_peak, in_rms) = lin.take();
            let (out_peak, out_rms) = lout.take();
            let buffered_ms = io.out.buffered_ms();
            let _ = app.emit("meter", MeterPayload { in_peak, in_rms, out_peak, out_rms, buffered_ms });
        }

        thread::park_timeout(Duration::from_millis(5));
    }
}
