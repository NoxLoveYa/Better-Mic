//! NVIDIA Maxine Audio Effects (NvAFX) denoiser, loaded at runtime from an installed SDK.
//! Looks in `%NVAFX_SDK_DIR%` and the default SDK install folder. Requires an RTX GPU.

use libloading::os::windows::{Library, LOAD_WITH_ALTERED_SEARCH_PATH};
use serde::Serialize;
use std::ffi::{c_char, c_void, CString};
use std::path::{Path, PathBuf};

use super::FRAME;

type Handle = *mut c_void;
type CreateFn = unsafe extern "C" fn(*const c_char, *mut Handle) -> i32;
type SetU32Fn = unsafe extern "C" fn(Handle, *const c_char, u32) -> i32;
type SetF32Fn = unsafe extern "C" fn(Handle, *const c_char, f32) -> i32;
type SetStrFn = unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> i32;
type GetU32Fn = unsafe extern "C" fn(Handle, *const c_char, *mut u32) -> i32;
type LoadFn = unsafe extern "C" fn(Handle) -> i32;
type RunFn = unsafe extern "C" fn(Handle, *const *const f32, *const *mut f32, u32, u32) -> i32;
type DestroyFn = unsafe extern "C" fn(Handle) -> i32;

#[derive(Serialize)]
pub struct Status {
    pub available: bool,
    pub detail: String,
}

fn sdk_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = ["NVAFX_SDK_DIR", "AFX_SDK_DIR"].iter().filter_map(|k| std::env::var(k).ok()).map(PathBuf::from).collect();
    let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into());
    v.push(PathBuf::from(pf).join(r"NVIDIA Corporation\NVIDIA Audio Effects SDK"));
    v
}

fn models_in(sdk: &Path) -> Vec<PathBuf> {
    let root = sdk.join("models");
    let mut out = vec![root.join("denoiser_48k.trtpkg")];
    if let Ok(rd) = std::fs::read_dir(&root) {
        out.extend(rd.flatten().map(|e| e.path().join("denoiser_48k.trtpkg")));
    }
    out.retain(|p| p.is_file());
    out
}

fn find() -> Option<(PathBuf, Vec<PathBuf>)> {
    sdk_dirs().into_iter().find_map(|d| {
        let models = models_in(&d);
        (d.join("NVAudioEffects.dll").is_file() && !models.is_empty()).then_some((d, models))
    })
}

pub fn probe() -> Status {
    if let Some((d, _)) = find() {
        return Status { available: true, detail: format!("SDK found at {}", d.display()) };
    }
    let detail = match sdk_dirs().into_iter().find(|d| d.is_dir()) {
        Some(d) => format!(
            "Found {} but NVAudioEffects.dll or models\\denoiser_48k.trtpkg is missing. Reinstall the NVIDIA Audio Effects redistributable for your GPU.",
            d.display()
        ),
        None => "NVIDIA Audio Effects SDK is not installed (NVIDIA Broadcast doesn't include it). Install the \"Audio Effects\" redistributable for your GPU generation, the same one OBS uses: nvidia.com/en-us/geforce/broadcasting/broadcast-sdk/resources".into(),
    };
    Status { available: false, detail }
}

#[derive(Serialize)]
pub struct Gpu {
    pub name: String,
    /// Which redistributable build fits this GPU: turing, ampere, ada or blackwell.
    pub arch: &'static str,
}

fn arch_for(compute_cap: &str) -> Option<&'static str> {
    let (major, minor) = compute_cap.trim().split_once('.')?;
    match (major.parse::<u32>().ok()?, minor.parse::<u32>().ok()?) {
        (7, 5) => Some("turing"),
        (8, 9) => Some("ada"),
        (8, _) => Some("ampere"),
        (10 | 12, _) => Some("blackwell"),
        _ => None,
    }
}

/// First installed NVIDIA GPU with tensor cores that the Audio Effects SDK supports (RTX 20 series and newer).
pub fn detect_gpu() -> Option<Gpu> {
    use std::os::windows::process::CommandExt;
    let out = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=name,compute_cap", "--format=csv,noheader"])
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).lines().find_map(|l| {
        let (name, cap) = l.rsplit_once(',')?;
        Some(Gpu { name: name.trim().into(), arch: arch_for(cap)? })
    })
}

/// NVIDIA's own download for the Audio Effects redistributable (the build OBS uses).
pub fn installer_url(arch: &str) -> Option<String> {
    let build = match arch {
        "turing" => "Turing",
        "ampere" => "Ampere",
        "ada" => "Ada",
        "blackwell" => "Blackwell",
        _ => return None,
    };
    Some(format!(
        "https://international.download.nvidia.com/Windows/broadcast/sdk/AFX/2025-01-21_NVIDIA_AFX_SDK_Win_v1.6.1.2-GA_{build}.exe"
    ))
}

pub struct Nvidia {
    _lib: Library,
    h: Handle,
    run: RunFn,
    destroy: DestroyFn,
    set_f32: SetF32Fn,
}

// The handle is only ever used from the processing thread that owns the filter.
unsafe impl Send for Nvidia {}

impl Nvidia {
    pub fn open(intensity: f32) -> Result<Self, String> {
        let (dir, models) = find().ok_or("NVIDIA Audio Effects SDK not found")?;
        let extra: Vec<PathBuf> = ["", r"external\cuda\bin", r"external\cudnn\bin"].iter().map(|s| dir.join(s)).collect();
        let old = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(extra.into_iter().chain(std::env::split_paths(&old))).map_err(|e| e.to_string())?;
        std::env::set_var("PATH", path);

        unsafe {
            let lib = Library::load_with_flags(dir.join("NVAudioEffects.dll"), LOAD_WITH_ALTERED_SEARCH_PATH)
                .map_err(|e| format!("load NVAudioEffects.dll: {e}"))?;
            let create: CreateFn = *lib.get::<CreateFn>(b"NvAFX_CreateEffect\0").map_err(|e| e.to_string())?;
            let set_u32: SetU32Fn = *lib.get::<SetU32Fn>(b"NvAFX_SetU32\0").map_err(|e| e.to_string())?;
            let set_f32: SetF32Fn = *lib.get::<SetF32Fn>(b"NvAFX_SetFloat\0").map_err(|e| e.to_string())?;
            let set_str: SetStrFn = *lib.get::<SetStrFn>(b"NvAFX_SetString\0").map_err(|e| e.to_string())?;
            let get_u32: GetU32Fn = *lib.get::<GetU32Fn>(b"NvAFX_GetU32\0").map_err(|e| e.to_string())?;
            let load: LoadFn = *lib.get::<LoadFn>(b"NvAFX_Load\0").map_err(|e| e.to_string())?;
            let run: RunFn = *lib.get::<RunFn>(b"NvAFX_Run\0").map_err(|e| e.to_string())?;
            let destroy: DestroyFn = *lib.get::<DestroyFn>(b"NvAFX_DestroyEffect\0").map_err(|e| e.to_string())?;

            let mut last = String::from("no model could be loaded");
            for model in models {
                let mut h: Handle = std::ptr::null_mut();
                if create(c"denoiser".as_ptr(), &mut h) != 0 {
                    last = "NvAFX_CreateEffect failed".into();
                    continue;
                }
                let m = CString::new(model.to_string_lossy().as_bytes()).map_err(|e| e.to_string())?;
                set_str(h, c"model_path".as_ptr(), m.as_ptr());
                // Sample-rate parameter name differs between SDK versions; unknown ones just return an error code.
                for k in [c"sample_rate", c"input_sample_rate", c"output_sample_rate"] {
                    set_u32(h, k.as_ptr(), 48000);
                }
                set_f32(h, c"intensity_ratio".as_ptr(), intensity);
                let rc = load(h);
                if rc != 0 {
                    destroy(h);
                    last = format!("NvAFX_Load failed ({rc}) for {}", model.display());
                    continue;
                }
                let mut n = 0u32;
                get_u32(h, c"num_input_samples_per_frame".as_ptr(), &mut n);
                if n as usize != FRAME {
                    destroy(h);
                    last = format!("unexpected NVIDIA frame size {n}");
                    continue;
                }
                return Ok(Self { _lib: lib, h, run, destroy, set_f32 });
            }
            Err(last)
        }
    }

    pub fn set_intensity(&mut self, v: f32) {
        unsafe { (self.set_f32)(self.h, c"intensity_ratio".as_ptr(), v) };
    }

    pub fn process(&mut self, input: &[f32], output: &mut [f32]) -> bool {
        let i = [input.as_ptr()];
        let o = [output.as_mut_ptr()];
        unsafe { (self.run)(self.h, i.as_ptr(), o.as_ptr(), FRAME as u32, 1) == 0 }
    }
}

impl Drop for Nvidia {
    fn drop(&mut self) {
        unsafe { (self.destroy)(self.h) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs against the real SDK; skipped on machines where it isn't installed.
    #[test]
    fn real_sdk_loads_and_processes() {
        if !probe().available {
            eprintln!("skipped: NVIDIA Audio Effects SDK not installed");
            return;
        }
        let t = std::time::Instant::now();
        let mut nv = Nvidia::open(1.0).expect("open");
        eprintln!("model load took {:?}", t.elapsed());

        let (mut input, mut output) = ([0f32; FRAME], [0f32; FRAME]);
        let t = std::time::Instant::now();
        for f in 0..200 {
            for (i, s) in input.iter_mut().enumerate() {
                *s = 0.3 * (2.0 * std::f32::consts::PI * 220.0 * (f * FRAME + i) as f32 / 48000.0).sin();
            }
            assert!(nv.process(&input, &mut output), "NvAFX_Run failed on frame {f}");
            assert!(output.iter().all(|s| s.is_finite()));
        }
        eprintln!("200 frames (2 s of audio) processed in {:?}", t.elapsed());
        nv.set_intensity(0.5);
    }
}
