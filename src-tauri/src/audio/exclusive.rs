//! A WASAPI exclusive-mode output for the virtual cable.
//!
//! Screen-share audio capture (Discord's, for one) records what other apps play through the shared Windows mixer.
//! A shared stream into the cable is part of that, so viewers hear the voice twice. Exclusive streams skip the
//! mixer and can't be captured that way, which is how a real virtual-mic driver behaves. cpal only does shared mode.

use rtrb::Consumer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    IAudioClient, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED,
    AUDCLNT_SHAREMODE_EXCLUSIVE, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0,
};
use windows::Win32::Media::KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE};
use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW, WaitForSingleObject,
};

#[derive(Clone, Copy)]
enum Fmt {
    I16,
    I24,
    I24In32,
    I32,
    F32,
}

impl Fmt {
    fn bytes(self) -> usize {
        match self {
            Fmt::I16 => 2,
            Fmt::I24 => 3,
            _ => 4,
        }
    }

    /// Writes one mono sample as `bytes()` little-endian bytes.
    fn encode(self, s: f32, out: &mut [u8; 4]) {
        let s = s.clamp(-1.0, 1.0);
        match self {
            Fmt::I16 => out[..2].copy_from_slice(&((s * 32767.0) as i16).to_le_bytes()),
            Fmt::I24 => out[..3].copy_from_slice(&(((s * 8_388_607.0) as i32).to_le_bytes()[..3])),
            Fmt::I24In32 => *out = (((s * 8_388_607.0) as i32) << 8).to_le_bytes(),
            Fmt::I32 => *out = ((s as f64 * 2_147_483_647.0) as i32).to_le_bytes(),
            Fmt::F32 => *out = s.to_le_bytes(),
        }
    }
}

// (format, container bits, valid bits, float)
const FORMATS: [(Fmt, u16, u16, bool); 5] = [
    (Fmt::I24In32, 32, 24, false),
    (Fmt::I24, 24, 24, false),
    (Fmt::I16, 16, 16, false),
    (Fmt::I32, 32, 32, false),
    (Fmt::F32, 32, 32, true),
];

fn wave_format(rate: u32, ch: u16, bits: u16, valid: u16, float: bool) -> WAVEFORMATEXTENSIBLE {
    let block = ch * bits / 8;
    WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
            nChannels: ch,
            nSamplesPerSec: rate,
            nAvgBytesPerSec: rate * block as u32,
            nBlockAlign: block,
            wBitsPerSample: bits,
            cbSize: 22,
        },
        Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: valid },
        dwChannelMask: if ch == 1 { 0x4 } else { 0x3 },
        SubFormat: if float { KSDATAFORMAT_SUBTYPE_IEEE_FLOAT } else { KSDATAFORMAT_SUBTYPE_PCM },
    }
}

/// An initialised exclusive-mode client that isn't running yet.
pub struct Negotiated {
    client: IAudioClient,
    render: IAudioRenderClient,
    event: HANDLE,
    frames: u32,
    ch: usize,
    fmt: Fmt,
    pub rate: u32,
}

// COM interfaces and handles are raw pointers; these are only ever used from the one render thread after the move.
unsafe impl Send for Negotiated {}

impl Drop for Negotiated {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.event);
        }
    }
}

fn com_err(e: windows::core::Error) -> String {
    e.message()
}

/// Opens `id` (a cpal WASAPI device id) in exclusive mode with the first format it accepts.
/// Fails if another app has the device open, which is the usual reason this doesn't work.
pub fn negotiate(id: &str) -> Result<Negotiated, String> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(com_err)?;
        let wide: Vec<u16> = id.strip_prefix("wasapi:").unwrap_or(id).encode_utf16().chain([0]).collect();
        let dev = enumerator.GetDevice(PCWSTR(wide.as_ptr())).map_err(com_err)?;

        let mut failure = None;
        for rate in [48_000u32, 44_100, 96_000] {
            for ch in [2u16, 1] {
                for (fmt, bits, valid, float) in FORMATS {
                    let wfx = wave_format(rate, ch, bits, valid, float);
                    let client: IAudioClient = dev.Activate(CLSCTX_ALL, None).map_err(com_err)?;
                    if client.IsFormatSupported(AUDCLNT_SHAREMODE_EXCLUSIVE, &wfx.Format, None).is_err() {
                        continue;
                    }
                    match init(&dev, client, &wfx, rate) {
                        Ok(client) => return finish(client, ch as usize, fmt, rate),
                        Err(e) => failure = Some(com_err(e)),
                    }
                }
            }
        }
        Err(failure.unwrap_or_else(|| "the device accepts no exclusive-mode format".into()))
    }
}

unsafe fn init(
    dev: &IMMDevice,
    client: IAudioClient,
    wfx: &WAVEFORMATEXTENSIBLE,
    rate: u32,
) -> windows::core::Result<IAudioClient> {
    let mut period = 0i64;
    client.GetDevicePeriod(Some(&mut period), None)?;
    let flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK;
    match client.Initialize(AUDCLNT_SHAREMODE_EXCLUSIVE, flags, period, period, &wfx.Format, None) {
        Ok(()) => Ok(client),
        // The buffer has to be a whole number of hardware frames: ask how many fit, then retry on a fresh client.
        Err(e) if e.code() == AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED => {
            let frames = client.GetBufferSize()?;
            let aligned = (10_000_000.0 / rate as f64 * frames as f64 + 0.5) as i64;
            let client: IAudioClient = dev.Activate(CLSCTX_ALL, None)?;
            client.Initialize(AUDCLNT_SHAREMODE_EXCLUSIVE, flags, aligned, aligned, &wfx.Format, None)?;
            Ok(client)
        }
        Err(e) => Err(e),
    }
}

unsafe fn finish(client: IAudioClient, ch: usize, fmt: Fmt, rate: u32) -> Result<Negotiated, String> {
    let event = CreateEventW(None, false, false, PCWSTR::null()).map_err(com_err)?;
    let built = (|| -> windows::core::Result<Negotiated> {
        client.SetEventHandle(event)?;
        let frames = client.GetBufferSize()?;
        let render = client.GetService::<IAudioRenderClient>()?;
        Ok(Negotiated { client, render, event, frames, ch, fmt, rate })
    })();
    built.map_err(|e| {
        let _ = CloseHandle(event);
        com_err(e)
    })
}

impl Negotiated {
    fn fill(&self, cons: &mut Consumer<f32>) -> windows::core::Result<()> {
        unsafe {
            let bytes = self.fmt.bytes();
            let ptr = self.render.GetBuffer(self.frames)?;
            let buf = std::slice::from_raw_parts_mut(ptr, self.frames as usize * self.ch * bytes);
            let mut enc = [0u8; 4];
            for frame in buf.chunks_exact_mut(self.ch * bytes) {
                self.fmt.encode(cons.pop().unwrap_or(0.0), &mut enc);
                for slot in frame.chunks_exact_mut(bytes) {
                    slot.copy_from_slice(&enc[..bytes]);
                }
            }
            self.render.ReleaseBuffer(self.frames, 0)
        }
    }

    fn run(self, mut cons: Consumer<f32>, stop: Arc<AtomicBool>, mut err: impl FnMut(String)) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let mut task = 0u32;
            let mmcss = AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut task).ok();

            // In exclusive event mode the whole buffer is filled before starting and again on every event.
            let started = self.fill(&mut cons).and_then(|_| self.client.Start());
            match started {
                Ok(()) => {
                    while !stop.load(Ordering::Relaxed) {
                        if WaitForSingleObject(self.event, 200) == WAIT_OBJECT_0 {
                            if let Err(e) = self.fill(&mut cons) {
                                err(format!("The virtual cable output stopped: {}", e.message()));
                                break;
                            }
                        }
                    }
                    let _ = self.client.Stop();
                }
                Err(e) => err(format!("Could not start the virtual cable output: {}", e.message())),
            }

            if let Some(h) = mmcss {
                let _ = AvRevertMmThreadCharacteristics(h);
            }
        }
    }
}

/// Plays whatever is pushed into the ring buffer's other end. Stops and joins its thread on drop.
pub struct Stream {
    parts: Option<(Negotiated, Consumer<f32>)>,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    err: Option<Box<dyn FnMut(String) + Send>>,
}

impl Stream {
    pub fn new(n: Negotiated, cons: Consumer<f32>, err: impl FnMut(String) + Send + 'static) -> Self {
        Self { parts: Some((n, cons)), stop: Arc::new(AtomicBool::new(false)), join: None, err: Some(Box::new(err)) }
    }

    pub fn play(&mut self) {
        if let (Some((n, cons)), Some(err)) = (self.parts.take(), self.err.take()) {
            let stop = self.stop.clone();
            self.join = Some(thread::spawn(move || n.run(cons, stop, err)));
        }
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use rtrb::RingBuffer;
    use std::sync::Mutex;
    use std::time::Duration;

    /// Loopback-captures what the shared Windows mixer carries for `dev`, i.e. what screen-share audio capture sees.
    fn loopback(dev: &cpal::Device) -> Result<(cpal::Stream, Arc<Mutex<(f64, u64)>>), String> {
        let cfg = dev.default_output_config().map_err(|e| e.to_string())?;
        let acc = Arc::new(Mutex::new((0f64, 0u64)));
        let a = acc.clone();
        let s = dev
            .build_input_stream(
                cfg.into(),
                move |d: &[f32], _| {
                    let mut g = a.lock().unwrap();
                    g.0 += d.iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
                    g.1 += d.len() as u64;
                },
                |_| {},
                None,
            )
            .map_err(|e| e.to_string())?;
        s.play().map_err(|e| e.to_string())?;
        Ok((s, acc))
    }

    fn rms(acc: &Mutex<(f64, u64)>) -> f64 {
        let g = acc.lock().unwrap();
        (g.0 / g.1.max(1) as f64).sqrt()
    }

    /// Run with: BM_TEST_DEVICE="CABLE In 16ch" cargo test exclusive_is_invisible_to_loopback -- --ignored --nocapture
    /// Plays a quiet 1 kHz tone, so only use a device nothing else is using.
    #[test]
    #[ignore]
    fn exclusive_is_invisible_to_loopback() {
        let want = std::env::var("BM_TEST_DEVICE").expect("set BM_TEST_DEVICE to part of an output device name").to_lowercase();
        let host = cpal::default_host();
        let dev = host
            .output_devices()
            .unwrap()
            .find(|d| d.description().map(|x| x.name().to_lowercase().contains(&want)).unwrap_or(false))
            .expect("no such output device");
        let id = dev.id().unwrap().to_string();
        println!("device: {} ({id})", dev.description().unwrap().name());

        // Control: a shared-mode stream is visible to loopback.
        let shared_rms = {
            let cfg = dev.default_output_config().unwrap();
            let ch = cfg.channels() as usize;
            let rate = cfg.sample_rate() as f32;
            let mut n = 0f32;
            let out = dev
                .build_output_stream(
                    cfg.into(),
                    move |d: &mut [f32], _| {
                        for f in d.chunks_mut(ch) {
                            f.fill(0.1 * (n * 2.0 * std::f32::consts::PI * 1000.0 / rate).sin());
                            n += 1.0;
                        }
                    },
                    |_| {},
                    None,
                )
                .unwrap();
            out.play().unwrap();
            let (_cap, acc) = loopback(&dev).unwrap();
            std::thread::sleep(Duration::from_millis(1000));
            rms(&acc)
        };
        println!("shared-mode tone, loopback rms:    {shared_rms:.4}");
        std::thread::sleep(Duration::from_millis(500));

        // Exclusive: same tone, and it must actually be consumed at the device rate.
        let n = match negotiate(&id) {
            Ok(n) => n,
            Err(e) => panic!("exclusive refused: {e}"),
        };
        let rate = n.rate;
        println!("exclusive format: {rate} Hz, {} ch", n.ch);
        let (mut prod, cons) = RingBuffer::<f32>::new(rate as usize / 2);
        let mut stream = Stream::new(n, cons, |e| println!("render error: {e}"));
        stream.play();
        let cap = loopback(&dev);
        let start = std::time::Instant::now();
        let (mut i, mut produced) = (0u64, 0u64);
        while start.elapsed() < Duration::from_millis(1000) {
            let s = 0.1 * (i as f32 * 2.0 * std::f32::consts::PI * 1000.0 / rate as f32).sin();
            if prod.push(s).is_ok() {
                i += 1;
                produced += 1;
            } else {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        let excl_rms = match &cap {
            Ok((_, acc)) => rms(acc),
            Err(e) => {
                println!("loopback could not even open while exclusive: {e}");
                0.0
            }
        };
        println!("exclusive tone, loopback rms:      {excl_rms:.4}");
        println!("exclusive samples pushed in 1 s:   {produced} (device rate {rate}, ring holds {})", rate / 2);
        drop(stream);

        assert!(shared_rms > 0.02, "control failed: loopback didn't hear the shared tone");
        assert!(excl_rms < shared_rms / 20.0, "loopback still hears the exclusive stream");
        // The ring starts empty and holds rate/2 samples, so what was pushed beyond that is what the device consumed.
        let consumed = produced.saturating_sub(rate as u64 / 2);
        assert!(consumed as f64 > rate as f64 * 0.8, "exclusive stream isn't consuming audio at the device rate");
    }
}
