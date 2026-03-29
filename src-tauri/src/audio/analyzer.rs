use std::fs::File;
use std::path::Path;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct AudioMetadata {
    pub duration_ms: i64,
    pub sample_rate: u32,
    pub channels: u16,
    pub bit_depth: u16,
}

pub fn extract_metadata(path: &Path) -> Result<AudioMetadata, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| format!("Failed to probe audio format: {}", e))?;

    let format = probed.format;

    let track = format.default_track().ok_or("No default audio track found")?;
    let codec_params = &track.codec_params;

    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    let channels = codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);
    let bit_depth = codec_params.bits_per_sample.unwrap_or(16) as u16;

    let duration_ms = if let (Some(n_frames), Some(tb)) = (codec_params.n_frames, codec_params.time_base) {
        let time = tb.calc_time(n_frames);
        (time.seconds as i64 * 1000) + (time.frac as f64 * 1000.0) as i64
    } else {
        0
    };

    Ok(AudioMetadata {
        duration_ms,
        sample_rate,
        channels,
        bit_depth,
    })
}