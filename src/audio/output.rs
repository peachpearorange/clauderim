use super::synth::{RATE, Wave};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Mix {
  pub gain: f32,
  pub pan: f32
}

impl Mix {
  pub const SILENT: Mix = Mix { gain: 0.0, pan: 0.0 };

  pub fn flat(gain: f32) -> Self { Self { gain, pan: 0.0 } }

  fn sides(self) -> [f32; 2] {
    let Mix { gain, pan } = self;
    [gain * (1.0 - pan.max(0.0) * 0.6), gain * (1.0 + pan.min(0.0) * 0.6)]
  }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Take(usize);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Voice(u64);

#[cfg(not(target_arch = "wasm32"))]
mod device {
  use {super::*,
       cpal::{BufferSize, FromSample, SampleFormat, SizedSample, StreamConfig,
              traits::{DeviceTrait, HostTrait, StreamTrait}},
       std::sync::{Arc,
                   mpsc::{Receiver, Sender, channel}}};

  const VOICES: usize = 64;
  const FRAMES: u32 = 1024;

  type Samples = Arc<Vec<Vec<f32>>>;

  enum Order {
    Start(Playing),
    Set(u64, Mix),
    Stop(u64)
  }

  struct Playing {
    id: u64,
    samples: Samples,
    at: f64,
    step: f64,
    looping: bool,
    now: [f32; 2],
    goal: [f32; 2]
  }

  impl Playing {
    fn length(&self) -> f64 { self.samples[0].len() as f64 }

    fn done(&self) -> bool { !self.looping && self.at >= self.length() - 1.0 }

    fn quiet(&self) -> bool {
      self.now.iter().chain(&self.goal).all(|gain| gain.abs() < 1e-4)
    }

    fn skip(&mut self, frames: usize) {
      self.at += self.step * frames as f64;
      if self.looping {
        self.at = self.at.rem_euclid(self.length())
      }
    }

    fn render(&mut self, into: &mut [[f32; 2]], glide: f32) {
      if self.quiet() {
        self.now = self.goal;
        self.skip(into.len())
      } else {
        let length = self.samples[0].len();
        let last = self.samples.len() - 1;
        into.iter_mut().for_each(|frame| {
          if !self.done() {
            let (index, blend) = (self.at as usize, self.at.fract() as f32);
            let next = (index + 1) % length;
            let read = |channel: &Vec<f32>| {
              channel[index % length] * (1.0 - blend) + channel[next] * blend
            };
            let (left, right) = (read(&self.samples[0]), read(&self.samples[last]));
            self.now = [0, 1]
              .map(|side| self.now[side] + (self.goal[side] - self.now[side]) * glide);
            frame[0] += left * self.now[0];
            frame[1] += right * self.now[1];
            self.skip(1)
          }
        })
      }
    }
  }

  struct Mixer {
    orders: Receiver<Order>,
    playing: Vec<Playing>,
    frames: Vec<[f32; 2]>,
    glide: f32
  }

  impl Mixer {
    fn fill<T: SizedSample + FromSample<f32>>(
      &mut self,
      data: &mut [T],
      channels: usize
    ) {
      self.orders.try_iter().for_each(|order| match order {
        Order::Start(playing) => self.playing.push(playing),
        Order::Set(id, mix) => self
          .playing
          .iter_mut()
          .filter(|playing| playing.id == id)
          .for_each(|playing| playing.goal = mix.sides()),
        Order::Stop(id) => self.playing.retain(|playing| playing.id != id)
      });
      let excess = self.playing.len().saturating_sub(VOICES);
      (0..excess).for_each(|_| {
        self
          .playing
          .iter()
          .position(|playing| !playing.looping)
          .map(|oldest| self.playing.remove(oldest));
      });
      self.frames.clear();
      self.frames.resize(data.len() / channels.max(1), [0.0; 2]);
      let glide = self.glide;
      self.playing.iter_mut().for_each(|playing| playing.render(&mut self.frames, glide));
      self.playing.retain(|playing| !playing.done());
      data.chunks_mut(channels.max(1)).zip(&self.frames).for_each(
        |(out, &[left, right])| {
          out.iter_mut().enumerate().for_each(|(channel, sample)| {
            let value = match (channels, channel) {
              (1, _) => (left + right) * 0.5,
              (_, 0) => left,
              (_, 1) => right,
              _ => 0.0
            };
            *sample = T::from_sample(value.clamp(-1.0, 1.0))
          })
        }
      )
    }
  }

  pub struct Output {
    orders: Sender<Order>,
    rate: f64,
    takes: Vec<Samples>,
    next: u64,
    _stream: cpal::Stream
  }

  impl Output {
    fn stream<T: SizedSample + FromSample<f32>>(
      device: &cpal::Device,
      config: &StreamConfig,
      orders: Receiver<Order>
    ) -> Option<cpal::Stream> {
      let channels = config.channels as usize;
      let mut mixer = Mixer {
        orders,
        playing: Vec::new(),
        frames: Vec::new(),
        glide: 1.0 - (-1.0 / (0.02 * config.sample_rate as f32)).exp()
      };
      device
        .build_output_stream(
          config,
          move |data: &mut [T], _| mixer.fill(data, channels),
          |error| bevy::log::warn!("audio stream: {error}"),
          None
        )
        .inspect_err(|error| bevy::log::warn!("audio stream: {error}"))
        .ok()
    }

    pub fn open() -> Option<Self> {
      let device = cpal::default_host().default_output_device()?;
      let supported = device.default_output_config().ok()?;
      let format = supported.sample_format();
      let base = supported.config();
      [StreamConfig { buffer_size: BufferSize::Fixed(FRAMES), ..base.clone() }, base]
        .into_iter()
        .find_map(|config| {
          let (orders, receiver) = channel();
          let stream = match format {
            SampleFormat::F32 => Self::stream::<f32>(&device, &config, receiver),
            SampleFormat::I16 => Self::stream::<i16>(&device, &config, receiver),
            SampleFormat::U16 => Self::stream::<u16>(&device, &config, receiver),
            SampleFormat::I32 => Self::stream::<i32>(&device, &config, receiver),
            _ => None
          }?;
          stream.play().ok()?;
          Some(Output {
            orders,
            rate: config.sample_rate as f64,
            takes: Vec::new(),
            next: 0,
            _stream: stream
          })
        })
    }

    pub fn load(&mut self, wave: Wave) -> Take {
      self.takes.push(Arc::new(wave.0));
      Take(self.takes.len() - 1)
    }

    pub fn start(&mut self, take: Take, mix: Mix, speed: f32, looping: bool) -> Voice {
      self.next += 1;
      let sides = mix.sides();
      self
        .orders
        .send(Order::Start(Playing {
          id: self.next,
          samples: self.takes[take.0].clone(),
          at: 0.0,
          step: RATE as f64 / self.rate * speed as f64,
          looping,
          now: sides,
          goal: sides
        }))
        .ok();
      Voice(self.next)
    }

    pub fn set(&mut self, voice: Voice, mix: Mix) {
      self.orders.send(Order::Set(voice.0, mix)).ok();
    }

    pub fn stop(&mut self, voice: Voice) { self.orders.send(Order::Stop(voice.0)).ok(); }
  }
}

#[cfg(target_arch = "wasm32")]
mod device {
  use {super::*,
       bevy::platform::collections::HashMap,
       web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext, GainNode,
                 StereoPannerNode}};

  struct Playing {
    source: AudioBufferSourceNode,
    gain: GainNode,
    panner: StereoPannerNode
  }

  pub struct Output {
    context: AudioContext,
    takes: Vec<AudioBuffer>,
    playing: HashMap<u64, Playing>,
    next: u64
  }

  impl Output {
    pub fn open() -> Option<Self> {
      AudioContext::new().ok().map(|context| Output {
        context,
        takes: Vec::new(),
        playing: HashMap::default(),
        next: 0
      })
    }

    pub fn load(&mut self, wave: Wave) -> Take {
      let frames = wave.0.first().map(Vec::len).unwrap_or(0).max(1);
      if let Ok(buffer) =
        self.context.create_buffer(wave.0.len() as u32, frames as u32, RATE)
      {
        wave.0.iter().enumerate().for_each(|(channel, samples)| {
          buffer.copy_to_channel(samples, channel as i32).ok();
        });
        self.takes.push(buffer)
      }
      Take(self.takes.len().saturating_sub(1))
    }

    fn apply(&self, playing: &Playing, mix: Mix, glide: f64) {
      let now = self.context.current_time();
      playing.gain.gain().set_target_at_time(mix.gain, now, glide).ok();
      playing
        .panner
        .pan()
        .set_target_at_time(mix.pan.clamp(-1.0, 1.0) * 0.6, now, glide)
        .ok();
    }

    pub fn start(&mut self, take: Take, mix: Mix, speed: f32, looping: bool) -> Voice {
      self.next += 1;
      if let Some(buffer) = self.takes.get(take.0)
        && let Ok(source) = self.context.create_buffer_source()
        && let Ok(gain) = self.context.create_gain()
        && let Ok(panner) = self.context.create_stereo_panner()
      {
        source.set_buffer(Some(buffer));
        source.set_loop(looping);
        source.playback_rate().set_value(speed);
        gain.gain().set_value(mix.gain);
        panner.pan().set_value(mix.pan.clamp(-1.0, 1.0) * 0.6);
        source.connect_with_audio_node(&gain).ok();
        gain.connect_with_audio_node(&panner).ok();
        panner.connect_with_audio_node(&self.context.destination()).ok();
        source.start().ok();
        self.playing.insert(self.next, Playing { source, gain, panner });
      }
      Voice(self.next)
    }

    pub fn set(&mut self, voice: Voice, mix: Mix) {
      self.playing.get(&voice.0).map(|playing| self.apply(playing, mix, 0.03));
    }

    pub fn stop(&mut self, voice: Voice) {
      self.playing.remove(&voice.0).map(|Playing { source, panner, .. }| {
        source.stop().ok();
        panner.disconnect().ok()
      });
    }
  }
}

pub use device::Output;
