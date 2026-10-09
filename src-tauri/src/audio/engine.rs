//! Mic capture -> DSP chain -> virtual cable. cpal streams are `!Send`, so everything lives on one
//! dedicated thread; the audio callbacks only move samples through lock-free ring buffers.

use super::find_device;
use super::resample::Resampler;
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

struct Io {
    _input: Stream,
    _output: Stream,
    in_cons: Consumer<f32>,
    out_prod: Producer<f32>,
    in_rate: u32,
    out_rate: u32,
    out_cap: usize,
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
    let out_dev = find_device(output, false)?;
    let in_sup = in_dev.default_input_config().map_err(|e| e.to_string())?;
    let out_sup = out_dev.default_output_config().map_err(|e| e.to_string())?;
    let (in_rate, out_rate) = (in_sup.sample_rate(), out_sup.sample_rate());

    let (in_prod, in_cons) = RingBuffer::<f32>::new(in_rate as usize / 2);
    let out_cap = out_rate as usize / 2;
    let (out_prod, out_cons) = RingBuffer::<f32>::new(out_cap);

    let wake = thread::current();
    let err_in = err_cb(app);
    let err_out = err_cb(app);
    let input_stream = match in_sup.sample_format() {
        SampleFormat::F32 => build_input::<f32>(&in_dev, in_sup.clone().into(), in_prod, wake, err_in),
        SampleFormat::I16 => build_input::<i16>(&in_dev, in_sup.clone().into(), in_prod, wake, err_in),
        SampleFormat::I32 => build_input::<i32>(&in_dev, in_sup.clone().into(), in_prod, wake, err_in),
        f => Err(format!("unsupported input sample format {f}")),
    }?;
    let output_stream = match out_sup.sample_format() {
        SampleFormat::F32 => build_output::<f32>(&out_dev, out_sup.clone().into(), out_cons, err_out),
        SampleFormat::I16 => build_output::<i16>(&out_dev, out_sup.clone().into(), out_cons, err_out),
        SampleFormat::I32 => build_output::<i32>(&out_dev, out_sup.clone().into(), out_cons, err_out),
        f => Err(format!("unsupported output sample format {f}")),
    }?;

    let mut io = Io { _input: input_stream, _output: output_stream, in_cons, out_prod, in_rate, out_rate, out_cap };
    // Pre-fill with silence so the output callback doesn't underrun while the first frames are processed.
    for _ in 0..(out_rate as f32 * TARGET_BUFFER_MS / 1000.0) as usize {
        let _ = io.out_prod.push(0.0);
    }
    io._output.play().map_err(|e| e.to_string())?;
    io._input.play().map_err(|e| e.to_string())?;
    Ok(io)
}

fn err_cb(app: &AppHandle) -> impl FnMut(cpal::Error) + Send + 'static {
    let app = app.clone();
    move |e| {
        let _ = app.emit("engine-error", e.to_string());
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
    let mut out_rs = Resampler::new(SR as f64, io.out_rate as f64);
    let base_step = out_rs.step;
    let target_fill = io.out_rate as f32 * TARGET_BUFFER_MS / 1000.0;
    let mut fill_avg = target_fill;

    let (mut raw, mut acc) = (Vec::<f32>::new(), Vec::<f32>::new());
    let mut frame = [0f32; FRAME];
    let (mut lin, mut lout) = (Level::default(), Level::default());
    let mut last_emit = Instant::now();

    loop {
        while let Ok(m) = rx.try_recv() {
            match m {
                Msg::Chain(c) => chain.apply(c),
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

            // Nudge the output resampler to hold the buffer at its target fill despite clock drift.
            let fill = (io.out_cap - io.out_prod.slots()) as f32;
            fill_avg += (fill - fill_avg) * 0.02;
            let err = ((fill_avg - target_fill) / target_fill).clamp(-1.0, 1.0) as f64;
            out_rs.step = base_step * (1.0 + 0.005 * err);
            out_rs.push(&frame, |s| {
                let _ = io.out_prod.push(s);
            });
        }
        acc.drain(..off);

        if last_emit.elapsed() >= Duration::from_millis(33) {
            last_emit = Instant::now();
            let (in_peak, in_rms) = lin.take();
            let (out_peak, out_rms) = lout.take();
            let buffered_ms = fill_avg / io.out_rate as f32 * 1000.0 + 1000.0 * FRAME as f32 / SR;
            let _ = app.emit("meter", MeterPayload { in_peak, in_rms, out_peak, out_rms, buffered_ms });
        }

        thread::park_timeout(Duration::from_millis(5));
    }
}
