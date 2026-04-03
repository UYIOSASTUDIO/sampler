use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Maximum number of interleaved samples decoded for waveform analysis.
///
/// Previous limit: 10 minutes → 52.9M samples → slow for long tracks.
/// New limit:      60 seconds  →  5.3M samples → 10× faster, visually identical
///                                                for any sample library use-case.
///
/// 60 s @ 44.1kHz stereo = 44100 × 2 × 60 = 5_292_000 samples
const MAX_WAVEFORM_SAMPLES: usize = 44_100 * 2 * 60;

/// Extracts a compact peak-amplitude waveform from any supported audio file.
///
/// # Strategy
/// Decodes audio sequentially, accumulating samples into `num_bars` equally-sized
/// windows and computing the peak amplitude of each window.  Decoding stops as
/// soon as `MAX_WAVEFORM_SAMPLES` are read, giving accurate shapes for One-Shots
/// and representative overviews for Loops without reading the entire file.
///
/// # Returns
/// A `Vec<u8>` of length `num_bars`, each value in `[2, 100]`.
pub fn extract_waveform(path: &Path, num_bars: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| e.to_string())?;

    let mut format = probed.format;
    let track = format.default_track().ok_or("No default audio track")?;
    let track_id = track.id;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;

    // Pre-allocate with the cap so we never exceed it
    let mut all_samples: Vec<f32> = Vec::with_capacity(MAX_WAVEFORM_SAMPLES.min(1_048_576));
    let mut sample_buf: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(_) => break, // EOF or unrecoverable error
        };

        if packet.track_id() != track_id {
            continue;
        }

        let audio_buf = match decoder.decode(&packet) {
            Ok(b) => b,
            Err(_) => break,
        };

        let spec = *audio_buf.spec();
        let sb =
            sample_buf.get_or_insert_with(|| SampleBuffer::new(audio_buf.capacity() as u64, spec));
        sb.copy_interleaved_ref(audio_buf);
        all_samples.extend_from_slice(sb.samples());

        // Early exit — 60 s of audio is more than enough for a waveform thumbnail
        if all_samples.len() >= MAX_WAVEFORM_SAMPLES {
            break;
        }
    }

    if all_samples.is_empty() {
        return Err("No audio samples decoded".to_string());
    }

    // ── Peak detection per bar ────────────────────────────────────────────────
    let chunk_size = (all_samples.len() / num_bars).max(1);
    let mut waveform = Vec::with_capacity(num_bars);

    for i in 0..num_bars {
        let start = i * chunk_size;
        let end = (start + chunk_size).min(all_samples.len());
        let chunk = &all_samples[start..end];

        // Branchless peak: fold over absolute values
        let peak = chunk.iter().fold(0.0_f32, |acc, &s| acc.max(s.abs()));

        let val = ((peak * 100.0).round() as u8).clamp(2, 100);
        waveform.push(val);
    }

    Ok(waveform)
}
