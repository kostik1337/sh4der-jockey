use rodio::{decoder::DecoderError, Decoder, OutputStream, Sink, Source};
use std::{
    fs::File,
    io::BufReader,
    sync::{Arc, Mutex},
    time::Duration,
};

/// Maximum number of seconds the audio can be out of sync by
/// before the source decides to jump to the target.
const JUMP_THRESHOLD: f64 = 0.3;

/// Weight for interpolating the playback time towards the target.
///
/// Setting this too high will introduce sampling artefacts.
/// If set too low, the audio might drift out of sync.
const TIME_LERP: f64 = 1e-6;

/// ~~that's what they called me in college~~
/// Weight for interpolating the playback speed towards the target.
const SPEED_LERP: f64 = 0.5;

pub struct Playback {
    handle: Arc<Mutex<Option<(f64, f64)>>>,
    _sink: Sink,
    // music stops when this thing drops
    _stream: OutputStream,
}

impl Playback {
    pub fn new() -> Option<Self> {
        let (stream, stream_handle) = OutputStream::try_default().unwrap();
        let sink = Sink::try_new(&stream_handle).unwrap();

        let (source, handle) = RemoteSource::from_file(File::open("test.mp3").unwrap()).unwrap();

        sink.append(source);

        Some(Self {
            handle,
            _stream: stream,
            _sink: sink,
        })
    }

    pub fn resync(&self, time: f64, speed: f64) {
        *self.handle.lock().unwrap() = Some((time, speed));
    }
}

struct RemoteSource {
    data: Vec<i16>,
    control: Arc<Mutex<Option<(f64, f64)>>>,
    time: f64,
    speed: f64,
    sample_rate: u32,
    channels: u16,
}

impl RemoteSource {
    pub fn from_file(file: File) -> Result<(Self, Arc<Mutex<Option<(f64, f64)>>>), DecoderError> {
        let decoder = Decoder::new(BufReader::new(file)).unwrap();
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();
        let data = decoder.collect();

        let control = Default::default();
        let control_handle = Arc::clone(&control);

        let this = Self {
            data,
            control,
            time: 0.0,
            speed: 1.0,
            sample_rate,
            channels,
        };

        Ok((this, control_handle))
    }
}

impl Iterator for RemoteSource {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        // TODO: Don't implicitely convert to mono
        let sample = (self.time * self.sample_rate as f64).round() as usize;
        let index = (self.channels as usize * sample + 1) % self.data.len();
        let value = self.data.get(index).cloned();

        // fetch target and drop the mutex right away
        let target = self.control.lock().unwrap().take();

        // nudge the internal state towards the target
        if let Some((target_time, target_speed)) = target {
            let speed_delta = target_speed - self.speed;
            self.speed += SPEED_LERP * speed_delta;

            let time_delta = target_time - self.time;
            if time_delta.abs() > JUMP_THRESHOLD {
                println!("Seek audio by {}s", time_delta);
                self.time = target_time;
            } else {
                self.time += TIME_LERP * time_delta;
            }
        }

        self.time += self.speed / (self.sample_rate as f64 * self.channels as f64);
        value
    }
}

impl Source for RemoteSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
