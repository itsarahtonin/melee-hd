//! The output device through WASAPI directly (Windows), for the smallest buffers it allows,
//! which cpal can't ask for. Shared, the stream asks the audio engine for its smallest period
//! (`IAudioClient3::InitializeSharedAudioStream`, available since Windows 10 where the driver
//! supports it, often 2 to 3 ms), else the engine's current period if another program holds it,
//! else the standard 10 ms; it keeps two periods queued on the device. Exclusive, it takes the
//! device itself at its smallest period, in a format the device plays natively, and no other
//! program's sound plays until it lets go. A thread of its own, raised to the multimedia
//! scheduler's "Pro Audio" class, fills the device's buffer at each period's event.
//!
//! The thread keeps the device: the system tells it of every device added, removed or changed
//! and of a new default (`IMMNotificationClient`), and when the device it plays on is no longer
//! the one it should (the one the options name while it's there, else the default), or fails
//! (`AUDCLNT_E_DEVICE_INVALIDATED` once it's unplugged, or its events stop), it opens the right
//! one and plays on.

use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};
use windows::Win32::Foundation::{CloseHandle, HANDLE, PROPERTYKEY, WAIT_OBJECT_0};
use windows::Win32::Media::Audio::{
    AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED, AUDCLNT_SHAREMODE_EXCLUSIVE,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, DEVICE_STATE, EDataFlow, ERole,
    IAudioClient, IAudioClient3, IAudioClock, IAudioRenderClient, IMMDevice, IMMDeviceEnumerator,
    IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator, WAVE_FORMAT_PCM,
    WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0, eConsole, eRender,
};
use windows::Win32::Media::KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE};
use windows::Win32::Media::Multimedia::{KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, WAVE_FORMAT_IEEE_FLOAT};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize,
};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Threading::{
    AvRevertMmThreadCharacteristics, AvSetMmThreadCharacteristicsW, CreateEventW,
    WaitForSingleObject,
};
use windows::core::{HSTRING, Interface, PCWSTR, implement, w};

use super::{Control, Info, OutputOptions, Player, Resampler, Ring, put_frame};

/// Starts the output's thread on the device `control`'s options name (else the default).
pub(super) fn spawn(ring: &Arc<Ring>, control: &Arc<Control>) -> Result<(Player, Info), String> {
    let (ring, ctl) = (ring.clone(), control.clone());
    Player::spawn(control, move |tx| {
        // SAFETY: COM is initialized once on this thread and closed at its end; every interface
        // below is used on this thread only, but the notification client, which only sets
        // flags.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            keep(&ring, &ctl, tx);
            CoUninitialize();
        }
    })
}

/// Tells the output's thread of every change to the system's devices.
#[implement(IMMNotificationClient)]
struct Notify(Arc<Control>);

impl IMMNotificationClient_Impl for Notify_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        self.0.recheck.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.0.recheck.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.0.recheck.store(true, Ordering::Relaxed);
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        if flow == eRender && role == eConsole {
            self.0.recheck.store(true, Ordering::Relaxed);
        }
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}

/// The output's thread: opens the device it should play on and plays the ring on it until
/// asked to stop, opening another whenever that device fails or stops being the one. The first
/// device opened, or why none could be, goes to `first`.
///
/// # Safety
/// COM is initialized on this thread.
unsafe fn keep(
    ring: &Arc<Ring>,
    control: &Arc<Control>,
    first: mpsc::Sender<Result<Info, String>>,
) {
    // SAFETY: plain COM calls on this thread's objects.
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(e) => {
                    let _ = first.send(Err(format!("the device enumerator: {e}")));
                    return;
                }
            };
        let notify: IMMNotificationClient = Notify(control.clone()).into();
        let listening = enumerator
            .RegisterEndpointNotificationCallback(&notify)
            .is_ok();
        let mut first = Some(first);
        let mut said = false;
        while !control.stopping() {
            control.recheck.store(false, Ordering::Relaxed);
            let options = control.options();
            if first.is_none() {
                ring.restart();
            }
            match Device::open(&enumerator, &options) {
                Ok(mut d) => {
                    said = false;
                    control.opened.fetch_add(1, Ordering::Relaxed);
                    let info = d.info();
                    match first.take() {
                        Some(tx) => {
                            let _ = tx.send(Ok(info));
                        }
                        None => eprintln!("sound: now {} Hz, {}", info.rate, info.describe),
                    }
                    if let Err(e) = d.play(ring, control, &enumerator) {
                        eprintln!("sound: {e}; opening the output again");
                    }
                }
                Err(e) => {
                    if let Some(tx) = first.take() {
                        let _ = tx.send(Err(e));
                        break;
                    }
                    if !said {
                        eprintln!("sound: no output ({e}); trying again as the devices change");
                        said = true;
                    }
                    control.idle(Duration::from_secs(1));
                }
            }
        }
        if listening {
            let _ = enumerator.UnregisterEndpointNotificationCallback(&notify);
        }
    }
}

/// An endpoint's ID.
///
/// # Safety
/// COM is initialized on this thread.
unsafe fn id_of(device: &IMMDevice) -> Result<String, String> {
    // SAFETY: the ID is a string COM allocated, read and freed once.
    unsafe {
        let p = device.GetId().map_err(err("the device's ID"))?;
        let id = p.to_string().map_err(|e| format!("the device's ID: {e}"));
        CoTaskMemFree(Some(p.0 as *const _));
        id
    }
}

/// The device `options` name while it's there (found by name among the active outputs, as
/// cpal lists them: WASAPI's endpoints and their friendly names), else the default, with its
/// ID and name.
///
/// # Safety
/// COM is initialized on this thread.
unsafe fn endpoint(
    enumerator: &IMMDeviceEnumerator,
    options: &OutputOptions,
) -> Result<(IMMDevice, String, String), String> {
    // SAFETY: plain COM calls.
    unsafe {
        let named = |want: &str| -> Option<(IMMDevice, String)> {
            let mut all = cpal::default_host().output_devices().ok()?;
            let id = all.find(|d| d.to_string() == want)?.id().ok()?;
            let device = enumerator.GetDevice(&HSTRING::from(id.id())).ok()?;
            Some((device, id.id().to_owned()))
        };
        if let Some(want) = &options.device
            && let Some((device, id)) = named(want)
        {
            return Ok((device, id, want.clone()));
        }
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(err("no default audio output device"))?;
        let id = id_of(&device)?;
        let name = cpal::default_host()
            .output_devices()
            .ok()
            .and_then(|mut all| all.find(|d| d.id().is_ok_and(|i| i.id() == id)))
            .map_or_else(|| "the default device".to_owned(), |d| d.to_string());
        Ok((device, id, name))
    }
}

/// The device's sample format.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Sample {
    F32,
    I16,
    /// 32-bit words, of which `bits` are valid (24 or 32).
    I32 {
        bits: u16,
    },
}

impl Sample {
    fn bytes(self) -> u16 {
        match self {
            Sample::I16 => 2,
            _ => 4,
        }
    }

    fn name(self) -> String {
        match self {
            Sample::F32 => "float".into(),
            Sample::I16 => "16-bit".into(),
            Sample::I32 { bits } => format!("{bits}-bit"),
        }
    }
}

/// A format: the structure WASAPI takes, and what it means.
#[derive(Clone, Copy)]
struct Format {
    wave: WAVEFORMATEXTENSIBLE,
    sample: Sample,
    channels: usize,
    rate: u32,
}

impl Format {
    /// A format of our own making.
    fn new(rate: u32, channels: u16, sample: Sample) -> Format {
        let bytes = sample.bytes();
        let block = bytes * channels;
        let wave = WAVEFORMATEXTENSIBLE {
            Format: WAVEFORMATEX {
                wFormatTag: WAVE_FORMAT_EXTENSIBLE as u16,
                nChannels: channels,
                nSamplesPerSec: rate,
                nAvgBytesPerSec: rate * u32::from(block),
                nBlockAlign: block,
                wBitsPerSample: bytes * 8,
                cbSize: (size_of::<WAVEFORMATEXTENSIBLE>() - size_of::<WAVEFORMATEX>()) as u16,
            },
            Samples: WAVEFORMATEXTENSIBLE_0 {
                wValidBitsPerSample: match sample {
                    Sample::I32 { bits } => bits,
                    _ => bytes * 8,
                },
            },
            // Front left and right, then whatever follows.
            dwChannelMask: if channels == 2 { 0x3 } else { 0 },
            SubFormat: if sample == Sample::F32 {
                KSDATAFORMAT_SUBTYPE_IEEE_FLOAT
            } else {
                KSDATAFORMAT_SUBTYPE_PCM
            },
        };
        Format {
            wave,
            sample,
            channels: usize::from(channels),
            rate,
        }
    }

    /// The format WASAPI hands back (the engine's mix format), copied, and freed.
    ///
    /// # Safety
    /// `p` is a `WAVEFORMATEX` (or the extensible one its tag says) from `CoTaskMemAlloc`.
    unsafe fn take(p: *mut WAVEFORMATEX) -> Result<Format, String> {
        // SAFETY: as the caller promises; the extensible part is read only when the tag says
        // it's there.
        unsafe {
            let base = p.read_unaligned();
            let mut wave = WAVEFORMATEXTENSIBLE {
                Format: base,
                ..Default::default()
            };
            let tag = u32::from(base.wFormatTag);
            let (float, valid) = if tag == WAVE_FORMAT_EXTENSIBLE {
                wave = p.cast::<WAVEFORMATEXTENSIBLE>().read_unaligned();
                let sub = wave.SubFormat;
                let samples = wave.Samples;
                (
                    sub == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT,
                    samples.wValidBitsPerSample,
                )
            } else {
                (tag == WAVE_FORMAT_IEEE_FLOAT, base.wBitsPerSample)
            };
            CoTaskMemFree(Some(p as *const _));
            let pcm = tag == WAVE_FORMAT_PCM || (tag == WAVE_FORMAT_EXTENSIBLE && !float);
            let sample = match (float, pcm, base.wBitsPerSample) {
                (true, _, 32) => Sample::F32,
                (_, true, 16) => Sample::I16,
                (_, true, 32) => Sample::I32 { bits: valid },
                _ => {
                    let bits = base.wBitsPerSample;
                    return Err(format!(
                        "the mix format (tag {tag:#x}, {bits} bits) isn't one the player writes"
                    ));
                }
            };
            Ok(Format {
                wave,
                sample,
                channels: usize::from(base.nChannels),
                rate: base.nSamplesPerSec,
            })
        }
    }

    fn ptr(&self) -> *const WAVEFORMATEX {
        &self.wave.Format
    }
}

/// 100-nanosecond units to and from device samples.
fn hns_to_frames(hns: i64, rate: u32) -> u32 {
    ((hns * i64::from(rate) + 5_000_000) / 10_000_000) as u32
}

fn frames_to_hns(frames: u32, rate: u32) -> i64 {
    (i64::from(frames) * 10_000_000 + i64::from(rate) / 2) / i64::from(rate)
}

fn err(what: &str) -> impl Fn(windows::core::Error) -> String + '_ {
    move |e| format!("{what}: {e}")
}

/// An opened device stream.
struct Device {
    client: IAudioClient,
    render: IAudioRenderClient,
    clock: IAudioClock,
    event: HANDLE,
    format: Format,
    exclusive: bool,
    /// The period the events come at, and the device buffer, in device samples.
    period: u32,
    buffer: u32,
    /// The device samples kept queued (shared mode), and the engine's own latency.
    queue: u32,
    stream_latency: Duration,
    how: String,
    /// The endpoint's ID and name, and the options it was opened by.
    id: String,
    name: String,
    options: OutputOptions,
}

impl Device {
    /// The device `options` name (else the default), opened by them.
    ///
    /// # Safety
    /// COM is initialized on this thread.
    unsafe fn open(
        enumerator: &IMMDeviceEnumerator,
        options: &OutputOptions,
    ) -> Result<Device, String> {
        // SAFETY: plain COM calls on interfaces this thread made.
        unsafe {
            let (device, id, name) = endpoint(enumerator, options)?;
            let opened = match options.exclusive {
                true => match Device::open_exclusive(&device) {
                    Ok(d) => Ok(d),
                    Err(e) => {
                        eprintln!("sound: exclusive mode: {e}; sharing the device");
                        Device::open_shared(&device)
                    }
                },
                false => Device::open_shared(&device),
            };
            opened.map(|mut d| {
                d.id = id;
                d.name = name;
                d.options = options.clone();
                d
            })
        }
    }

    /// The engine's smallest period, or its current one, or the standard way.
    unsafe fn open_shared(device: &IMMDevice) -> Result<Device, String> {
        // SAFETY: plain COM calls; the formats outlive the calls that read them.
        unsafe {
            let client3: IAudioClient3 = device
                .Activate(CLSCTX_ALL, None)
                .map_err(err("the audio client"))?;
            let format = Format::take(client3.GetMixFormat().map_err(err("the mix format"))?)?;
            let (mut default, mut fundamental, mut min, mut max) = (0, 0, 0, 0);
            let periods = client3
                .GetSharedModeEnginePeriod(
                    format.ptr(),
                    &mut default,
                    &mut fundamental,
                    &mut min,
                    &mut max,
                )
                .is_ok();
            let mut current = 0;
            let mut current_format = std::ptr::null_mut();
            if client3
                .GetCurrentSharedModeEnginePeriod(&mut current_format, &mut current)
                .is_ok()
            {
                CoTaskMemFree(Some(current_format as *const _));
            }
            let flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK;
            let low = if periods && min > 0 && min < default {
                [min, current]
                    .into_iter()
                    .filter(|&p| p > 0 && p < default)
                    .find(|&p| {
                        client3
                            .InitializeSharedAudioStream(flags, p, format.ptr(), None)
                            .is_ok()
                    })
            } else {
                None
            };
            let (client, period, how) = match low {
                Some(p) => (
                    client3
                        .cast::<IAudioClient>()
                        .map_err(err("IAudioClient"))?,
                    p,
                    format!(
                        "WASAPI shared, low-latency engine period (smallest {min}, standard {default})"
                    ),
                ),
                None => {
                    // A client whose initialization failed can't be initialized again.
                    let client: IAudioClient = device
                        .Activate(CLSCTX_ALL, None)
                        .map_err(err("the audio client"))?;
                    client
                        .Initialize(AUDCLNT_SHAREMODE_SHARED, flags, 0, 0, format.ptr(), None)
                        .map_err(err("initializing the shared stream"))?;
                    let mut hns = 0;
                    client
                        .GetDevicePeriod(Some(&mut hns), None)
                        .map_err(err("the device period"))?;
                    let why = if periods && min < default {
                        "the engine's period is held at the standard by another program"
                    } else {
                        "the driver has no smaller engine period"
                    };
                    (
                        client,
                        hns_to_frames(hns, format.rate),
                        format!("WASAPI shared, standard engine period ({why})"),
                    )
                }
            };
            Device::finish(client, format, false, period, how)
        }
    }

    /// The device itself at its smallest period, in the first format it takes natively.
    unsafe fn open_exclusive(device: &IMMDevice) -> Result<Device, String> {
        // SAFETY: plain COM calls; the formats outlive the calls that read them.
        unsafe {
            let client: IAudioClient = device
                .Activate(CLSCTX_ALL, None)
                .map_err(err("the audio client"))?;
            let mix = Format::take(client.GetMixFormat().map_err(err("the mix format"))?)?;
            let (mut default, mut min) = (0i64, 0i64);
            client
                .GetDevicePeriod(Some(&mut default), Some(&mut min))
                .map_err(err("the device period"))?;
            let mut rates = vec![mix.rate, 48_000, 44_100];
            rates.dedup();
            let mut channels = vec![2, mix.channels as u16];
            channels.dedup();
            let samples = [
                Sample::F32,
                Sample::I32 { bits: 32 },
                Sample::I32 { bits: 24 },
                Sample::I16,
            ];
            let mut candidates = Vec::new();
            for &r in &rates {
                for &c in &channels {
                    candidates.extend(samples.iter().map(|&s| Format::new(r, c, s)));
                }
            }
            let format = candidates
                .into_iter()
                .find(|f| {
                    client
                        .IsFormatSupported(AUDCLNT_SHAREMODE_EXCLUSIVE, f.ptr(), None)
                        .is_ok()
                })
                .ok_or("the device takes none of the formats the player writes")?;
            let flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK;
            let init = |c: &IAudioClient, hns: i64| {
                c.Initialize(
                    AUDCLNT_SHAREMODE_EXCLUSIVE,
                    flags,
                    hns,
                    hns,
                    format.ptr(),
                    None,
                )
            };
            let client = match init(&client, min) {
                Ok(()) => client,
                Err(e) if e.code() == AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED => {
                    // The period the device can do near the smallest, on a new client.
                    let frames = client.GetBufferSize().map_err(err("the buffer size"))?;
                    let client: IAudioClient = device
                        .Activate(CLSCTX_ALL, None)
                        .map_err(err("the audio client"))?;
                    init(&client, frames_to_hns(frames, format.rate))
                        .map_err(err("initializing the exclusive stream"))?;
                    client
                }
                Err(e) => return Err(format!("initializing the exclusive stream: {e}")),
            };
            let period = client.GetBufferSize().map_err(err("the buffer size"))?;
            let how = format!(
                "WASAPI exclusive (no other program's sound plays; smallest device period {:.2} ms)",
                min as f64 / 10_000.0
            );
            Device::finish(client, format, true, period, how)
        }
    }

    /// The event, the services and the sizes of an initialized client.
    unsafe fn finish(
        client: IAudioClient,
        format: Format,
        exclusive: bool,
        period: u32,
        how: String,
    ) -> Result<Device, String> {
        // SAFETY: plain COM and kernel calls on this thread's objects.
        unsafe {
            let event = CreateEventW(None, false, false, None).map_err(err("an event"))?;
            client
                .SetEventHandle(event)
                .map_err(err("the stream's event"))?;
            let buffer = client.GetBufferSize().map_err(err("the buffer size"))?;
            let render: IAudioRenderClient =
                client.GetService().map_err(err("the render client"))?;
            let clock: IAudioClock = client.GetService().map_err(err("the audio clock"))?;
            let hns = client.GetStreamLatency().unwrap_or(0);
            Ok(Device {
                client,
                render,
                clock,
                event,
                format,
                exclusive,
                period: period.max(1),
                buffer,
                queue: buffer.min(2 * period.max(1)),
                stream_latency: Duration::from_nanos(hns.max(0) as u64 * 100),
                how,
                id: String::new(),
                name: String::new(),
                options: OutputOptions::default(),
            })
        }
    }

    fn info(&self) -> Info {
        let rate = self.format.rate;
        let ms = |n: u32| f64::from(n) * 1000.0 / f64::from(rate);
        Info {
            rate,
            buffer: Some(self.period),
            describe: format!(
                "{}, {}-sample period ({:.2} ms), {} kept queued of a {}-sample buffer, {} Hz {} x{}, on {}",
                self.how,
                self.period,
                ms(self.period),
                if self.exclusive {
                    "one period".to_owned()
                } else {
                    format!("{} samples", self.queue)
                },
                self.buffer,
                rate,
                self.format.sample.name(),
                self.format.channels,
                self.name
            ),
        }
    }

    /// Whether this is still the device to play on, by the options as they stand.
    ///
    /// # Safety
    /// COM is initialized on this thread.
    unsafe fn wanted(&self, control: &Control, enumerator: &IMMDeviceEnumerator) -> bool {
        let options = control.options();
        // SAFETY: plain COM calls.
        let id = unsafe { endpoint(enumerator, &options) }.map(|e| e.1);
        options.exclusive == self.options.exclusive && id.as_deref() == Ok(self.id.as_str())
    }

    /// Plays the ring until asked to stop or the device stops being the one to play on (`Ok`),
    /// or until it fails (`Err`).
    unsafe fn play(
        &mut self,
        ring: &Arc<Ring>,
        control: &Control,
        enumerator: &IMMDeviceEnumerator,
    ) -> Result<(), String> {
        // SAFETY: plain COM and kernel calls on this thread's objects; each buffer WASAPI hands
        // out is written within the frames asked for, in its format, before it's released.
        unsafe {
            let mut task = 0;
            let mmcss = AvSetMmThreadCharacteristicsW(w!("Pro Audio"), &mut task).ok();
            let mut resampler = Resampler::new(ring.clone(), self.format.rate);
            let freq = self.clock.GetFrequency().map_err(err("the clock's rate"))?;
            let mut qpc = 0;
            let _ = QueryPerformanceFrequency(&mut qpc);
            let rate = f64::from(self.format.rate);
            // Silence first: a period exclusive, the queue shared.
            let first = if self.exclusive {
                self.buffer
            } else {
                self.queue
            };
            self.render
                .GetBuffer(first)
                .map_err(err("the device's buffer"))?;
            self.render
                .ReleaseBuffer(first, AUDCLNT_BUFFERFLAGS_SILENT.0 as u32)
                .map_err(err("the device's buffer"))?;
            let mut written = u64::from(first);
            self.client.Start().map_err(err("starting the stream"))?;
            let result = loop {
                if control.stopping() {
                    break Ok(());
                }
                #[cfg(test)]
                if control.failed() {
                    break Err("the device went away (a test's)".to_owned());
                }
                if control.recheck.swap(false, Ordering::Relaxed)
                    && !self.wanted(control, enumerator)
                {
                    eprintln!("sound: the output device changed");
                    break Ok(());
                }
                if control.lost.swap(false, Ordering::Relaxed) {
                    break Err("the device stopped".to_owned());
                }
                if WaitForSingleObject(self.event, 200) != WAIT_OBJECT_0 {
                    // No period for 200 ms: the device may be gone, which only its calls say
                    // (AUDCLNT_E_DEVICE_INVALIDATED); it sends no more events.
                    if let Err(e) = self.client.GetCurrentPadding() {
                        break Err(format!("the device's queue: {e}"));
                    }
                    continue;
                }
                let frames = if self.exclusive {
                    self.buffer
                } else {
                    match self.client.GetCurrentPadding() {
                        Ok(p) => self.queue.saturating_sub(p),
                        Err(e) => break Err(format!("the device's queue: {e}")),
                    }
                };
                if frames == 0 {
                    continue;
                }
                // When the first sample written now is heard: what the device still holds
                // (less what it played since it took the position, which moves a period at a
                // time), and the engine's latency.
                let (mut position, mut at) = (0, 0);
                if self.clock.GetPosition(&mut position, Some(&mut at)).is_ok() && freq > 0 {
                    let mut now = 0;
                    let since = if QueryPerformanceCounter(&mut now).is_ok() && qpc > 0 {
                        (now as f64 / qpc as f64 - at as f64 / 1e7).max(0.0)
                    } else {
                        0.0
                    };
                    let held = written as f64 / rate - position as f64 / freq as f64 - since;
                    let d = Duration::from_secs_f64(held.max(0.0)) + self.stream_latency;
                    resampler.ring.meters.device(d);
                }
                let data = match self.render.GetBuffer(frames) {
                    Ok(p) => p,
                    Err(e) => break Err(format!("the device's buffer: {e}")),
                };
                let n = frames as usize;
                let ch = self.format.channels;
                match self.format.sample {
                    Sample::F32 => write(&mut resampler, data.cast::<f32>(), n, ch),
                    Sample::I16 => write(&mut resampler, data.cast::<i16>(), n, ch),
                    Sample::I32 { .. } => write(&mut resampler, data.cast::<i32>(), n, ch),
                }
                if let Err(e) = self.render.ReleaseBuffer(frames, 0) {
                    break Err(format!("the device's buffer: {e}"));
                }
                written += u64::from(frames);
            };
            let _ = self.client.Stop();
            if let Some(h) = mmcss {
                let _ = AvRevertMmThreadCharacteristics(h);
            }
            result
        }
    }
}

/// Fills `frames` frames of `channels` samples at `data` from the resampler.
///
/// # Safety
/// `data` points at that many writable, aligned samples.
unsafe fn write<T: cpal::Sample + cpal::FromSample<f32>>(
    resampler: &mut Resampler,
    data: *mut T,
    frames: usize,
    channels: usize,
) {
    // SAFETY: as the caller promises.
    let out = unsafe { std::slice::from_raw_parts_mut(data, frames * channels) };
    let mut it = out.chunks_mut(channels.max(1));
    resampler.fill(frames, |s| {
        if let Some(frame) = it.next() {
            put_frame(frame, s);
        }
    });
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: the event is this device's, closed once.
        unsafe {
            let _ = CloseHandle(self.event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_convert() {
        assert_eq!(hns_to_frames(100_000, 48_000), 480);
        assert_eq!(hns_to_frames(26_667, 48_000), 128);
        assert_eq!(frames_to_hns(480, 48_000), 100_000);
        assert_eq!(frames_to_hns(144, 48_000), 30_000);
    }

    #[test]
    fn formats_describe_themselves() {
        let f = Format::new(48_000, 2, Sample::I32 { bits: 24 });
        let base = f.wave.Format;
        assert_eq!({ base.nBlockAlign }, 8);
        assert_eq!({ base.nAvgBytesPerSec }, 384_000);
        assert_eq!({ base.cbSize }, 22);
        let samples = f.wave.Samples;
        assert_eq!(unsafe { samples.wValidBitsPerSample }, 24);
    }
}
