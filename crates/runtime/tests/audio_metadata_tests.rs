use std::f32::consts::PI;
use std::time::Duration;

use vnengine_runtime::{audio_duration, MemoryAssetStore};

#[test]
fn audio_duration_reports_decodable_wav_and_fails_for_invalid_audio() {
    let mut store = MemoryAssetStore::default();
    let wav = tiny_wav(Duration::from_millis(250), 8_000);
    store.insert("tone.wav", wav);
    store.insert("broken.ogg", b"not audio".to_vec());

    let duration = audio_duration(&store, "tone.wav")
        .expect("valid wav duration")
        .expect("wav should report duration");
    assert!(
        (duration.as_secs_f32() - 0.25).abs() < 0.02,
        "duration should be close to 250ms, got {duration:?}"
    );

    let err = audio_duration(&store, "broken.ogg").expect_err("invalid audio should fail");
    assert!(err.contains("Failed to decode audio"));
}

fn tiny_wav(duration: Duration, sample_rate: u32) -> Vec<u8> {
    let samples = (duration.as_secs_f32() * sample_rate as f32).round() as u32;
    let data_bytes = samples * 2;
    let mut out = Vec::with_capacity(44 + data_bytes as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        let sample = (t * 440.0 * 2.0 * PI).sin() * i16::MAX as f32 * 0.1;
        out.extend_from_slice(&(sample as i16).to_le_bytes());
    }
    out
}
