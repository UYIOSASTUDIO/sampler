use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub fn extract_waveform(path: &Path, num_bars: usize) -> Result<String, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(|e| e.to_string())?;

    let mut format = probed.format;
    let track = format.default_track().ok_or("No default audio track")?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;

    let track_id = track.id;
    let mut all_samples = Vec::new();

    // Limit auf 10 Minuten (44.1kHz Stereo) setzen, damit komplette Tracks decodiert werden
    let max_samples = 44100 * 2 * 60 * 10;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => break, // EOF
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                let mut sample_buf = SampleBuffer::<f32>::new(audio_buf.capacity() as u64, *audio_buf.spec());
                sample_buf.copy_interleaved_ref(audio_buf);
                for sample in sample_buf.samples() {
                    all_samples.push(*sample);
                }
            }
            Err(_) => break,
        }

        if all_samples.len() > max_samples {
            break;
        }
    }

    if all_samples.is_empty() {
        return Err("No audio samples decoded".to_string());
    }

    let chunk_size = (all_samples.len() / num_bars).max(1);
    let mut waveform = Vec::with_capacity(num_bars);

    for i in 0..num_bars {
        let start = i * chunk_size;
        let end = usize::min(start + chunk_size, all_samples.len());
        let chunk = &all_samples[start..end];

        let mut peak = 0.0_f32;
        for &sample in chunk {
            let abs_sample = sample.abs();
            if abs_sample > peak {
                peak = abs_sample;
            }
        }

        let mut val = (peak * 100.0).round() as u8;
        if val < 2 { val = 2; }
        if val > 100 { val = 100; }
        waveform.push(val);
    }

    serde_json::to_string(&waveform).map_err(|e| e.to_string())
}