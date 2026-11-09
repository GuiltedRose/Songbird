use anyhow::{Context, Result, bail, ensure};
use flacenc::{
    component::{BitRepr, StreamInfo},
    error::Verify,
    source::{Fill, FrameBuf},
};
use md5::{Digest, Md5};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
/// Paths inserted into quoted composition text use portable forward slashes.
pub fn script_path(path: &Path) -> String {
    let value = path.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        value.replace('\\', "/")
    }
    #[cfg(not(windows))]
    {
        value
    }
}
static SERIAL: AtomicU64 = AtomicU64::new(0);
pub fn unique(dir: &Path, stem: &str, extension: &str) -> Result<PathBuf> {
    ensure!(
        !stem.is_empty()
            && stem
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "name must use letters, digits, - or _"
    );
    fs::create_dir_all(dir)?;
    let tick = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    Ok(dir.join(format!(
        "{stem}-{tick}-{}-{}.{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        extension
    )))
}
pub fn directory(root: &Path, relative: &str) -> Result<PathBuf> {
    let base = root.canonicalize()?;
    let mut current = base.clone();
    for part in Path::new(relative).components() {
        ensure!(
            matches!(part, std::path::Component::Normal(_)),
            "invalid project directory"
        );
        current.push(part);
        match fs::create_dir(&current) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        ensure!(
            current.canonicalize()?.starts_with(&base) && current.is_dir(),
            "project directory escapes root"
        );
    }
    Ok(current)
}
pub fn safe_path(root: &Path, path: &str) -> Result<PathBuf> {
    let rel = Path::new(path);
    ensure!(
        !rel.is_absolute()
            && !rel.components().any(|c| !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )),
        "asset must be a project-relative path"
    );
    let base = root.canonicalize()?;
    let full = base
        .join(rel)
        .canonicalize()
        .with_context(|| format!("missing asset: {path}"))?;
    ensure!(full.starts_with(&base), "asset escapes project directory");
    ensure!(full.is_file(), "asset is not a file");
    Ok(full)
}
pub struct Asset {
    file: File,
    pub rate: u32,
    pub channels: u16,
    pub frames: u64,
    pub peak: f32,
    pub waveform: Vec<f32>,
    pub path: PathBuf,
    page: Vec<[f32; 2]>,
    page_start: u64,
    page_len: usize,
}
impl Asset {
    pub fn decode(path: &Path, cache: &Path) -> Result<Self> {
        Self::decode_bounded(path, cache, 691_200_000)
    }
    pub(crate) fn decode_bounded(path: &Path, cache: &Path, max_samples: u64) -> Result<Self> {
        let target = unique(cache, "pcm", "raw")?;
        let mut writer = BufWriter::new(
            OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&target)?,
        );
        let mut count = 0u64;
        let mut peak = 0f32;
        let mut waveform = vec![];
        let mut chunk_peak = 0f32;
        let mut push = |x: f32| -> Result<()> {
            ensure!(x.is_finite(), "nonfinite audio sample");
            ensure!(
                count < max_samples,
                "decoded asset exceeds resource limit of {max_samples} interleaved samples"
            );
            peak = peak.max(x.abs());
            chunk_peak = chunk_peak.max(x.abs());
            if count % 2048 == 2047 {
                waveform.push(chunk_peak);
                chunk_peak = 0.;
            }
            writer.write_all(&x.to_le_bytes())?;
            count += 1;
            Ok(())
        };
        let ext = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let (rate, channels) = match ext.as_str() {
            "wav" => {
                let mut r = hound::WavReader::open(path)?;
                let s = r.spec();
                ensure!(
                    (1..=2).contains(&s.channels) && (8000..=96000).contains(&s.sample_rate),
                    "WAV requires mono/stereo, 8..96 kHz"
                );
                match s.sample_format {
                    hound::SampleFormat::Float => {
                        ensure!(s.bits_per_sample == 32, "unsupported WAV float depth");
                        for x in r.samples::<f32>() {
                            push(x?)?;
                        }
                    }
                    hound::SampleFormat::Int => {
                        ensure!(
                            (8..=32).contains(&s.bits_per_sample),
                            "unsupported WAV depth"
                        );
                        let scale = 2f32.powi(s.bits_per_sample as i32 - 1);
                        for x in r.samples::<i32>() {
                            push(x? as f32 / scale)?;
                        }
                    }
                }
                (s.sample_rate, s.channels)
            }
            "flac" => {
                let mut r = claxon::FlacReader::open(path)?;
                let s = r.streaminfo();
                ensure!(
                    (1..=2).contains(&s.channels) && (8000..=96000).contains(&s.sample_rate),
                    "FLAC requires mono/stereo, 8..96 kHz"
                );
                let scale = 2f32.powi(s.bits_per_sample as i32 - 1);
                for x in r.samples() {
                    push(x? as f32 / scale)?;
                }
                (s.sample_rate, s.channels as u16)
            }
            _ => bail!("expected WAV or FLAC: {}", path.display()),
        };
        writer.flush()?;
        ensure!(
            count > 0 && count.is_multiple_of(channels as u64),
            "empty or incomplete audio"
        );
        if chunk_peak > 0. {
            waveform.push(chunk_peak);
        }
        Ok(Self {
            file: File::open(&target)?,
            rate,
            channels,
            frames: count / channels as u64,
            peak,
            waveform,
            path: target,
            page: Vec::new(),
            page_start: u64::MAX,
            page_len: 0,
        })
    }
    pub fn frame(&mut self, index: i64) -> Result<[f32; 2]> {
        if index < 0 || index as u64 >= self.frames {
            return Ok([0.; 2]);
        }
        let index = index as u64;
        if self.page_start == u64::MAX
            || index < self.page_start
            || index >= self.page_start + self.page_len as u64
        {
            self.page.resize(8192, [0.; 2]);
            self.page_start = index / 8192 * 8192;
            self.page_len = (self.frames - self.page_start).min(8192) as usize;
            self.file
                .seek(SeekFrom::Start(self.page_start * self.channels as u64 * 4))?;
            let mut bytes = vec![0u8; self.page_len * self.channels as usize * 4];
            self.file.read_exact(&mut bytes)?;
            for (i, f) in self.page[..self.page_len].iter_mut().enumerate() {
                for (ch, x) in f.iter_mut().enumerate() {
                    let j = (i * self.channels as usize + ch.min(self.channels as usize - 1)) * 4;
                    *x = f32::from_le_bytes(bytes[j..j + 4].try_into()?);
                }
            }
        }
        Ok(self.page[(index - self.page_start) as usize])
    }
    // Windowed sinc interpolation, including antialiasing for downward resampling.
    pub fn sample(&mut self, pos: f64, speed: f64) -> Result<[f32; 2]> {
        let center = pos.floor() as i64;
        let cut = 1. / speed.abs().max(1.);
        let mut out = [0.; 2];
        let mut total = 0.;
        for k in -12..=12 {
            let d = (center + k) as f64 - pos;
            let x = std::f64::consts::PI * d * cut;
            let sinc = if x.abs() < 1e-9 { 1. } else { x.sin() / x };
            let win = 0.5 + 0.5 * (std::f64::consts::PI * d / 13.).cos();
            let w = (sinc * win * cut) as f32;
            let f = self.frame(center + k)?;
            out[0] += f[0] * w;
            out[1] += f[1] * w;
            total += w;
        }
        if total.abs() > 1e-6 {
            out[0] /= total;
            out[1] /= total;
        }
        Ok(out)
    }
    pub fn duration(&self) -> f64 {
        self.frames as f64 / self.rate as f64
    }
}
impl Drop for Asset {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub struct Encoder {
    inner: Encoding,
    pub frames: u64,
    rate: u32,
    channels: u16,
    bits: u16,
}
enum Encoding {
    Wav(hound::WavWriter<std::io::BufWriter<File>>),
    Flac {
        file: File,
        info: StreamInfo,
        pending: Vec<i32>,
        number: usize,
        digest: Md5,
    },
}
impl Encoder {
    pub fn create(path: &Path, rate: u32, channels: u16, bits: u16) -> Result<Self> {
        ensure!(
            [1, 2].contains(&channels)
                && [16, 24].contains(&bits)
                && (8000..=96000).contains(&rate),
            "invalid encoding configuration"
        );
        let f = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .with_context(|| {
                format!(
                    "cannot create {}; existing files are protected",
                    path.display()
                )
            })?;
        let inner = match path.extension().and_then(|x| x.to_str()) {
            Some("wav") => Encoding::Wav(hound::WavWriter::new(
                BufWriter::new(f),
                hound::WavSpec {
                    channels,
                    sample_rate: rate,
                    bits_per_sample: bits,
                    sample_format: hound::SampleFormat::Int,
                },
            )?),
            Some("flac") => {
                let mut f = f;
                let mut info = StreamInfo::new(rate as usize, channels as usize, bits as usize)?;
                info.set_block_sizes(4096, 4096)?;
                info.set_frame_sizes(0, 0)?;
                let mut sink = flacenc::bitsink::ByteSink::new();
                info.write(&mut sink)?;
                f.write_all(b"fLaC\x80\x00\x00\x22")?;
                f.write_all(sink.as_slice())?;
                Encoding::Flac {
                    file: f,
                    info,
                    pending: Vec::with_capacity(8192),
                    number: 0,
                    digest: Md5::new(),
                }
            }
            _ => bail!("export extension must be .flac or .wav"),
        };
        Ok(Self {
            inner,
            frames: 0,
            rate,
            channels,
            bits,
        })
    }
    pub fn write(&mut self, frames: &[[f32; 2]]) -> Result<()> {
        let scale = 2f32.powi(self.bits as i32 - 1);
        for frame in frames {
            for &x in frame.iter().take(self.channels as usize) {
                ensure!(x.is_finite(), "nonfinite output");
                let n = (x.clamp(-1., 1. - 1. / scale) * scale).round() as i32;
                match &mut self.inner {
                    Encoding::Wav(w) => w.write_sample(n)?,
                    Encoding::Flac {
                        file,
                        info,
                        pending,
                        number,
                        digest,
                    } => {
                        digest.update(&n.to_le_bytes()[..(self.bits / 8) as usize]);
                        pending.push(n);
                        if pending.len() == 4096 * self.channels as usize {
                            Self::flush_flac(file, info, pending, number, self.channels)?;
                        }
                    }
                }
            }
            self.frames += 1;
        }
        Ok(())
    }
    fn flush_flac(
        file: &mut File,
        info: &mut StreamInfo,
        pending: &mut Vec<i32>,
        number: &mut usize,
        channels: u16,
    ) -> Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        let config = flacenc::config::Encoder::default()
            .into_verified()
            .map_err(|e| anyhow::anyhow!("FLAC config: {e:?}"))?;
        let mut frame = FrameBuf::with_size(channels as usize, 4096)?;
        frame
            .fill_interleaved(pending)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let mut encoded = flacenc::encode_fixed_size_frame(&config, &frame, *number, info)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        // Predictive Rice coding can be pathological on abrupt 24-bit residuals.
        // Bound the encoded representation before allocating a bit sink; verbatim
        // FLAC is the lossless, bounded fallback for an oversized prediction.
        let budget = pending.len() * info.bits_per_sample() + 4096;
        if encoded.count_bits() > budget {
            let mut fallback = flacenc::config::Encoder::default();
            fallback.subframe_coding.use_fixed = false;
            fallback.subframe_coding.use_lpc = false;
            let fallback = fallback
                .into_verified()
                .map_err(|e| anyhow::anyhow!("FLAC fallback: {e:?}"))?;
            encoded = flacenc::encode_fixed_size_frame(&fallback, &frame, *number, info)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        ensure!(
            encoded.count_bits() <= budget,
            "FLAC frame exceeds bounded PCM representation"
        );
        let mut sink = flacenc::bitsink::ByteSink::new();
        encoded.write(&mut sink)?;
        ensure!(
            sink.as_slice().len() <= budget.div_ceil(8),
            "FLAC encoder size estimate mismatch"
        );
        let size = sink.as_slice().len();
        let min = if *number == 0 {
            size
        } else {
            info.min_frame_size().min(size)
        };
        let max = info.max_frame_size().max(size);
        info.set_frame_sizes(min, max)?;
        file.write_all(sink.as_slice())?;
        *number += 1;
        pending.clear();
        Ok(())
    }
    pub fn finish(mut self) -> Result<()> {
        match &mut self.inner {
            Encoding::Wav(_) => {}
            Encoding::Flac {
                file,
                info,
                pending,
                number,
                digest,
            } => {
                let hash: [u8; 16] = digest.clone().finalize().into();
                info.set_md5_digest(&hash);
                Self::flush_flac(file, info, pending, number, self.channels)?;
                info.set_total_samples(self.frames as usize);
                let mut sink = flacenc::bitsink::ByteSink::new();
                info.write(&mut sink)?;
                file.seek(SeekFrom::Start(8))?;
                file.write_all(sink.as_slice())?;
                file.sync_all()?;
            }
        }
        if let Encoding::Wav(w) = self.inner {
            w.finalize()?;
        }
        Ok(())
    }
    pub fn rate(&self) -> u32 {
        self.rate
    }
}
pub fn convert(input: &Path, output: &Path, cache: &Path, rate: u32, bits: u16) -> Result<()> {
    let mut a = Asset::decode(input, cache)?;
    let mut e = Encoder::create(output, rate, a.channels, bits)?;
    let len = (a.duration() * rate as f64).round() as u64;
    let mut block = vec![];
    for i in 0..len {
        block.push(a.sample(
            i as f64 * a.rate as f64 / rate as f64,
            a.rate as f64 / rate as f64,
        )?);
        if block.len() == 1024 {
            e.write(&block)?;
            block.clear();
        }
    }
    e.write(&block)?;
    e.finish()
}

// Recover an interrupted PCM WAV into a new file; never modify the interrupted take.
pub fn recover_wav(input: &Path) -> Result<PathBuf> {
    let mut source = File::open(input)?;
    let len = source.metadata()?.len();
    ensure!(
        (44..=u32::MAX as u64).contains(&len),
        "unsupported WAV recovery size"
    );
    let mut header = [0u8; 12];
    source.read_exact(&mut header)?;
    ensure!(
        &header[..4] == b"RIFF" && &header[8..] == b"WAVE",
        "expected RIFF WAV"
    );
    let mut offset = 12u64;
    let mut align = 0u64;
    let mut data = None;
    while offset + 8 <= len {
        source.seek(SeekFrom::Start(offset))?;
        let mut chunk = [0u8; 8];
        source.read_exact(&mut chunk)?;
        let size = u32::from_le_bytes(chunk[4..].try_into()?) as u64;
        if &chunk[..4] == b"fmt " {
            ensure!((16..=4096).contains(&size), "invalid WAV format chunk");
            let mut fmt = vec![0u8; size as usize];
            source.read_exact(&mut fmt)?;
            ensure!(
                u16::from_le_bytes(fmt[..2].try_into()?) == 1
                    || (fmt.len() >= 40
                        && u16::from_le_bytes(fmt[..2].try_into()?) == 65534
                        && fmt[24..40]
                            == [1, 0, 0, 0, 0, 0, 16, 0, 128, 0, 0, 170, 0, 56, 155, 113]),
                "recovery supports PCM WAV"
            );
            align = u16::from_le_bytes(fmt[12..14].try_into()?) as u64;
        }
        if &chunk[..4] == b"data" {
            data = Some(offset);
            break;
        }
        offset = offset
            .checked_add(8 + size + (size % 2))
            .context("WAV size overflow")?;
    }
    let data = data.context("no WAV data chunk")?;
    ensure!(align > 0 && align <= 8, "invalid sample alignment");
    let samples = (len - data - 8) / align * align;
    ensure!(samples > 0, "no complete audio frames to recover");
    let new_len = data + 8 + samples;
    let output = unique(
        input.parent().context("missing parent")?,
        "recovered",
        "wav",
    )?;
    let mut out = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&output)?;
    source.seek(SeekFrom::Start(0))?;
    std::io::copy(&mut source.take(new_len), &mut out)?;
    out.seek(SeekFrom::Start(4))?;
    out.write_all(&((new_len - 8) as u32).to_le_bytes())?;
    out.seek(SeekFrom::Start(data + 4))?;
    out.write_all(&(samples as u32).to_le_bytes())?;
    out.sync_all()?;
    Ok(output)
}
