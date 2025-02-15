use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use super::Config;
use crate::util::RingBuffer;

pub const AUDIO_SAMPLES: usize = 8192;
pub const FFT_ATTACK: f32 = 0.5;
pub const FFT_DECAY: f32 = 0.5;

pub enum Channels {
    None,
    Mono,
    Stereo,
}

pub struct Audio {
    pub l_signal: Vec<f32>,
    pub r_signal: Vec<f32>,
    pub l_raw_spectrum: Vec<f32>,
    pub r_raw_spectrum: Vec<f32>,
    pub l_spectrum: Vec<f32>,
    pub r_spectrum: Vec<f32>,
    pub l_spectrum_integrated: Vec<f32>,
    pub r_spectrum_integrated: Vec<f32>,
    pub l_spectrum_smooth: Vec<f32>,
    pub r_spectrum_smooth: Vec<f32>,
    pub l_spectrum_smooth_integrated: Vec<f32>,
    pub r_spectrum_smooth_integrated: Vec<f32>,
    pub size: usize,
    pub nice_size: usize,
    pub volume: [f32; 3],
    pub volume_integrated: [f32; 3],
    pub bass: [f32; 3],
    pub mid: [f32; 3],
    pub high: [f32; 3],
    pub bass_integrated: [f32; 3],
    pub mid_integrated: [f32; 3],
    pub high_integrated: [f32; 3],
    pub bass_smooth: [f32; 3],
    pub mid_smooth: [f32; 3],
    pub high_smooth: [f32; 3],
    pub bass_smooth_integrated: [f32; 3],
    pub mid_smooth_integrated: [f32; 3],
    pub high_smooth_integrated: [f32; 3],
    l_fft: Vec<Complex<f32>>,
    r_fft: Vec<Complex<f32>>,
    l_samples: Arc<Mutex<RingBuffer<f32>>>,
    r_samples: Arc<Mutex<RingBuffer<f32>>>,
    stream: Option<cpal::Stream>,
    channels: Channels,
    sample_freq: usize,
    pub attack: f32,
    pub decay: f32,
    fft: Arc<dyn Fft<f32>>,
}

impl Audio {
    pub fn new(window_size: usize, config: &Config) -> Self {
        let size = window_size;
        let spec_size = size / 2;
        let bands = 100;

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(size);

        let mut this = Self {
            size,
            nice_size: bands,
            l_signal: vec![0.0; size],
            r_signal: vec![0.0; size],
            l_fft: vec![Complex::new(0.0, 0.0); size],
            r_fft: vec![Complex::new(0.0, 0.0); size],
            volume: [0.0; 3],
            volume_integrated: [0.0; 3],
            bass: [0.0; 3],
            bass_integrated: [0.0; 3],
            mid: [0.0; 3],
            mid_integrated: [0.0; 3],
            high: [0.0; 3],
            high_integrated: [0.0; 3],
            bass_smooth: [0.0; 3],
            mid_smooth: [0.0; 3],
            high_smooth: [0.0; 3],
            bass_smooth_integrated: [0.0; 3],
            mid_smooth_integrated: [0.0; 3],
            high_smooth_integrated: [0.0; 3],
            l_raw_spectrum: vec![0.0; spec_size],
            r_raw_spectrum: vec![0.0; spec_size],
            l_spectrum: vec![0.0; bands],
            r_spectrum: vec![0.0; bands],
            l_spectrum_integrated: vec![0.0; bands],
            r_spectrum_integrated: vec![0.0; bands],
            l_spectrum_smooth: vec![0.0; bands],
            r_spectrum_smooth: vec![0.0; bands],
            l_spectrum_smooth_integrated: vec![0.0; bands],
            r_spectrum_smooth_integrated: vec![0.0; bands],
            l_samples: Arc::new(Mutex::new(RingBuffer::new(size))),
            r_samples: Arc::new(Mutex::new(RingBuffer::new(size))),
            stream: None,
            channels: Channels::None,
            fft,
            attack: 0.5,
            decay: 0.5,
            sample_freq: 0,
        };

        if let Err(err) = this.connect(config) {
            log::error!("Error connecting to audio input device: {}", err);
        }

        this
    }

    pub fn resize(&mut self, new_size: usize) {
        self.size = new_size;
        let spec_size = new_size / 2;

        let mut planner = FftPlanner::<f32>::new();
        self.fft = planner.plan_fft_forward(new_size);

        self.l_signal = vec![0.0; new_size];
        self.r_signal = vec![0.0; new_size];
        self.l_fft = vec![Complex::new(0.0, 0.0); new_size];
        self.r_fft = vec![Complex::new(0.0, 0.0); new_size];
        self.l_raw_spectrum = vec![0.0; spec_size];
        self.r_raw_spectrum = vec![0.0; spec_size];
        *self.l_samples.lock().unwrap() = RingBuffer::new(new_size);
        *self.r_samples.lock().unwrap() = RingBuffer::new(new_size);
    }

    pub fn connect(&mut self, config: &Config) -> Result<(), String> {
        let host = cpal::default_host();
        log::info!("Available Hosts: {:?}", cpal::available_hosts());
        let device = match &config.audio_device {
            None => host
                .default_input_device()
                .ok_or("No input device is available".to_string()),
            Some(s) => {
                let mut ret = None;
                for dev in host.input_devices().unwrap() {
                    let dev_name = dev.name().map_err(|e| e.to_string())?;
                    if dev_name.contains(s) {
                        ret = Some(dev);
                    }
                }
                ret.ok_or(format!("Failed to find audio device {}", s))
            }
        }?;

        log::info!(
            "Connected to audio input device: {:?}",
            device.name().unwrap_or("<no-name>".into())
        );

        let supported_configs_range = device
            .supported_input_configs()
            .map_err(|e| e.to_string())?;

        let supported_config = supported_configs_range
            .filter(|c| c.sample_format() == cpal::SampleFormat::F32)
            .next()
            .ok_or("no supported config?!".to_string())?
            .with_max_sample_rate();

        log::info!("Supported Config: {:?}", supported_config);

        let config = device
            .default_input_config()
            .map_err(|e| e.to_string())?
            .config();

        let sample_format = supported_config.sample_format();
        log::info!("Creating with config: {:?}", config);

        let channel_count = config.channels as usize;
        self.channels = match channel_count {
            1 => Channels::Mono,
            2 => Channels::Stereo,
            _ => Channels::None,
        };

        // TODO: receive config for FFT buffer size

        let l_samples_p = self.l_samples.clone();
        let r_samples_p = self.r_samples.clone();

        let input_callback = move |data: &[f32], _: &cpal::InputCallbackInfo| {
            {
                let mut l_samples_lock = l_samples_p.lock().unwrap();
                for x in data.iter().step_by(channel_count) {
                    l_samples_lock.push(x);
                }
            }

            if channel_count > 1 {
                let mut r_samples_lock = r_samples_p.lock().unwrap();
                for x in data.iter().skip(1).step_by(channel_count) {
                    r_samples_lock.push(x);
                }
            }
        };

        let stream = match sample_format {
            cpal::SampleFormat::F32 => device
                .build_input_stream(&config, input_callback, |err| {
                    // react to errors here.
                    log::error!("Failed to build input stream: {}", err);
                })
                .map_err(|_| "Failed to initialize audio input stream".to_string())?,
            s => return Err(format!("Unsupported sample format {:?}", s)),
        };

        stream.play().map_err(|e| e.to_string())?;

        let sample_freq = config.sample_rate.0;
        self.sample_freq = sample_freq as _;

        self.stream = Some(stream);
        Ok(())
    }

    pub fn update_samples(&mut self) {
        if self.stream.is_none() {
            return;
        }

        let l_samples_p = Arc::clone(&self.l_samples);
        let l_samples = l_samples_p.lock().unwrap();
        l_samples.copy_to_slice(&mut self.l_signal);

        // calculate volume with RMS
        self.volume[1] =
            (self.l_signal.iter().map(|&x| x.powi(2)).sum::<f32>() / l_samples.size as f32).sqrt();

        if let Channels::Stereo = self.channels {
            let r_samples_p = self.r_samples.clone();
            let r_samples = r_samples_p.lock().unwrap();
            r_samples.copy_to_slice(&mut self.r_signal);
            self.volume[2] = (self.r_signal.iter().map(|&x| x.powi(2)).sum::<f32>()
                / l_samples.size as f32)
                .sqrt();
            self.volume[0] = (self.volume[1] + self.volume[2]) / 2.0;
        } else {
            self.volume[0] = self.volume[1];
        };

        self.volume_integrated
            .iter_mut()
            .zip(self.volume.iter())
            .for_each(sum_left);
    }

    pub fn update_fft(&mut self) {
        if self.stream.is_none() {
            return;
        }

        let left_iter = self.l_signal.iter().map(|&x| Complex::new(x, 0.0));
        let right_iter = self.r_signal.iter().map(|&x| Complex::new(x, 0.0));

        fn fill_iter<T>(slice: &mut [T], mut iter: impl ExactSizeIterator<Item = T>) {
            debug_assert!(iter.len() >= slice.len());

            for element in slice {
                // This can be simplified as follows once `unwrap_unchecked` is stable:
                // *element = unsafe { iter.next().unwrap_unchecked() };
                match iter.next() {
                    Some(item) => *element = item,
                    None => unsafe { std::hint::unreachable_unchecked() },
                }
            }
        }

        fill_iter(&mut self.l_fft, left_iter);
        fill_iter(&mut self.r_fft, right_iter);

        self.fft.process(&mut self.l_fft);
        self.fft.process(&mut self.r_fft);

        let left_spectrum = self.l_fft.iter().map(|z| z.norm_sqr());
        let right_spectrum = self.r_fft.iter().map(|z| z.norm_sqr());

        fill_iter(&mut self.l_raw_spectrum, left_spectrum);
        fill_iter(&mut self.r_raw_spectrum, right_spectrum);

        debug_assert!(self.l_raw_spectrum.iter().all(|x| x.is_finite()));
        debug_assert!(self.r_raw_spectrum.iter().all(|x| x.is_finite()));

        self.update_nice_fft();
        self.update_smooth_fft();
        self.update_bass_mid_high();
    }

    fn update_nice_fft(&mut self) {
        if self.stream.is_none() {
            return;
        }
        let n = self.l_raw_spectrum.len() * 2;
        let bins = self.l_spectrum.len();

        self.l_spectrum.fill(0.0);
        self.r_spectrum.fill(0.0);
        self.bass = [0.0; 3];
        self.mid = [0.0; 3];
        self.high = [0.0; 3];

        let fs_over_n = self.sample_freq as f32 / n as f32;

        let half_n = self.l_raw_spectrum.len() as f32;
        let inv_half_n = 1.0 / half_n;

        let mut max_left: f32 = 0.0;
        let mut max_right: f32 = 0.0;
        for (i, (l, r)) in self
            .l_raw_spectrum
            .iter()
            .zip(self.r_raw_spectrum.iter())
            .enumerate()
        {
            let freq = i as f64 * fs_over_n as f64;

            // https://www.wikiwand.com/en/Piano_key_frequencies
            let bin = (12f64 * (freq / 440f64).log2()) as i32 + 49;
            let bi = if bin >= bins as _ {
                bins - 1
            } else if bin < 0 {
                0
            } else {
                bin as usize
            };

            // https://github.com/jberg/butterchurn/blob/master/src/audio/fft.js#L20
            let eq = -0.02 * ((half_n - i as f32) * inv_half_n).log10();
            let l_int = l * eq;
            let r_int = r * eq;
            max_left = max_left.max(l_int);
            max_right = max_right.max(r_int);

            self.l_spectrum[bi] = self.l_spectrum[bi].max(l_int);
            self.r_spectrum[bi] = self.r_spectrum[bi].max(r_int);
        }

        for i in 1..(bins - 1) {
            if self.l_spectrum[i] == 0.0 {
                self.l_spectrum[i] = (self.l_spectrum[i - 1] + self.l_spectrum[i + 1]) / 2.0;
            }
            if self.r_spectrum[i] == 0.0 {
                self.r_spectrum[i] = (self.r_spectrum[i - 1] + self.r_spectrum[i + 1]) / 2.0;
            }
        }

        for i in 0..bins {
            self.l_spectrum[i] /= if max_left == 0.0 { 1.0 } else { max_left };
            self.r_spectrum[i] /= if max_right == 0.0 { 1.0 } else { max_right };
        }

        self.l_spectrum_integrated
            .iter_mut()
            .zip(&self.l_spectrum)
            .for_each(sum_left);

        self.r_spectrum_integrated
            .iter_mut()
            .zip(&self.r_spectrum)
            .for_each(sum_left);
    }

    fn update_smooth_fft(&mut self) {
        let w_att_acc = self.attack;
        let w_att_val = 1.0 - w_att_acc;
        let w_dec_acc = self.decay;
        let w_dec_val = 1.0 - self.decay;

        let f = |(acc, val): (&mut f32, &f32)| {
            let mix = if val > &acc {
                *acc * w_att_acc + val * w_att_val
            } else {
                *acc * w_dec_acc + val * w_dec_val
            };
            *acc = mix;
        };

        self.l_spectrum_smooth
            .iter_mut()
            .zip(&self.l_spectrum)
            .for_each(f);

        self.r_spectrum_smooth
            .iter_mut()
            .zip(&self.r_spectrum)
            .for_each(f);

        self.l_spectrum_smooth_integrated
            .iter_mut()
            .zip(&self.l_spectrum_smooth)
            .for_each(sum_left);

        self.r_spectrum_smooth_integrated
            .iter_mut()
            .zip(&self.r_spectrum_smooth)
            .for_each(sum_left);
    }

    fn update_bass_mid_high(&mut self) {
        let bins = self.l_spectrum_smooth.len();

        self.bass_smooth = [0.0; 3];
        self.mid_smooth = [0.0; 3];
        self.high_smooth = [0.0; 3];
        for i in 0..bins {
            if i < 25 {
                self.bass_smooth[1] = self.bass_smooth[1].max(self.l_spectrum_smooth[i]);
                self.bass_smooth[2] = self.bass_smooth[2].max(self.r_spectrum_smooth[i]);
            } else if i < 80 {
                self.mid_smooth[1] = self.mid_smooth[1].max(self.l_spectrum_smooth[i]);
                self.mid_smooth[2] = self.mid_smooth[2].max(self.r_spectrum_smooth[i]);
            } else {
                self.high_smooth[1] = self.high_smooth[1].max(self.l_spectrum_smooth[i]);
                self.high_smooth[2] = self.high_smooth[2].max(self.r_spectrum_smooth[i]);
            }
        }
        self.bass_smooth[0] = (self.bass_smooth[1] + self.bass_smooth[2]) / 2.0;
        self.mid_smooth[0] = (self.mid_smooth[1] + self.mid_smooth[2]) / 2.0;
        self.high_smooth[0] = (self.high_smooth[1] + self.high_smooth[2]) / 2.0;

        for i in 0..bins {
            if i < 25 {
                self.bass[1] = self.bass[1].max(self.l_spectrum[i]);
                self.bass[2] = self.bass[2].max(self.r_spectrum[i]);
            } else if i < 80 {
                self.mid[1] = self.mid[1].max(self.l_spectrum[i]);
                self.mid[2] = self.mid[2].max(self.r_spectrum[i]);
            } else {
                self.high[1] = self.high[1].max(self.l_spectrum[i]);
                self.high[2] = self.high[2].max(self.r_spectrum[i]);
            }
        }
        self.bass[0] = (self.bass[1] + self.bass[2]) / 2.0;
        self.mid[0] = (self.mid[1] + self.mid[2]) / 2.0;
        self.high[0] = (self.high[1] + self.high[2]) / 2.0;

        self.bass_smooth_integrated
            .iter_mut()
            .zip(self.bass_smooth.iter())
            .for_each(sum_left);
        self.mid_smooth_integrated
            .iter_mut()
            .zip(self.mid_smooth.iter())
            .for_each(sum_left);
        self.high_smooth_integrated
            .iter_mut()
            .zip(self.high_smooth.iter())
            .for_each(sum_left);

        self.bass_integrated
            .iter_mut()
            .zip(self.bass.iter())
            .for_each(sum_left);
        self.mid_integrated
            .iter_mut()
            .zip(self.mid.iter())
            .for_each(sum_left);
        self.high_integrated
            .iter_mut()
            .zip(self.high.iter())
            .for_each(sum_left);
    }

    #[allow(dead_code)]
    pub fn get_samples(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.update_samples();
        left.copy_from_slice(&self.l_signal);
        right.copy_from_slice(&self.r_signal);
    }
}

fn sum_left((acc, val): (&mut f32, &f32)) {
    *acc += val;
}
