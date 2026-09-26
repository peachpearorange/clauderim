use std::f32::consts::{PI, TAU};

pub const RATE: f32 = 44100.0;

pub fn len(secs: f32) -> usize { (secs.max(0.0) * RATE) as usize }

pub fn time(index: usize) -> f32 { index as f32 / RATE }

pub fn midi(note: f32) -> f32 { 440.0 * ((note - 69.0) / 12.0).exp2() }

pub fn smooth(x: f32) -> f32 {
  let x = x.clamp(0.0, 1.0);
  x * x * (3.0 - 2.0 * x)
}

pub fn drive(x: f32, amount: f32) -> f32 { (x * amount).tanh() / amount.tanh() }

pub fn perc(t: f32, attack: f32, decay: f32) -> f32 {
  if t < 0.0 {
    0.0
  } else if t < attack {
    t / attack
  } else {
    (-(t - attack) / decay).exp()
  }
}

#[derive(Clone, Copy)]
pub struct Adsr {
  pub attack: f32,
  pub decay: f32,
  pub sustain: f32,
  pub release: f32
}

impl Adsr {
  pub fn tail(&self) -> f32 { self.release * 5.0 }

  fn on(&self, t: f32) -> f32 {
    let &Adsr { attack, decay, sustain, .. } = self;
    if t < attack {
      smooth(t / attack)
    } else {
      sustain + (1.0 - sustain) * (-(t - attack) / decay).exp()
    }
  }

  pub fn level(&self, t: f32, held: f32) -> f32 {
    if t < held { self.on(t) } else { self.on(held) * (-(t - held) / self.release).exp() }
  }
}

pub fn curve(t: f32, points: &[(f32, f32)]) -> f32 {
  points
    .windows(2)
    .find(|pair| t < pair[1].0)
    .map(|pair| {
      let ((t0, v0), (t1, v1)) = (pair[0], pair[1]);
      v0 + (v1 - v0) * ((t - t0) / (t1 - t0).max(1e-6)).clamp(0.0, 1.0)
    })
    .unwrap_or(points[points.len() - 1].1)
}

#[derive(Clone)]
pub struct Rng(u64);

impl Default for Rng {
  fn default() -> Self { Self::new(0x51ce) }
}

impl Rng {
  pub fn new(seed: u64) -> Self {
    Self(
      seed.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x2545_F491_4F6C_DD1D
        | 1
    )
  }

  pub fn unit(&mut self) -> f32 {
    self.0 ^= self.0 << 13;
    self.0 ^= self.0 >> 7;
    self.0 ^= self.0 << 17;
    (self.0 >> 40) as f32 / 16_777_216.0
  }

  pub fn signed(&mut self) -> f32 { self.unit() * 2.0 - 1.0 }

  pub fn range(&mut self, low: f32, high: f32) -> f32 { low + (high - low) * self.unit() }

  pub fn below(&mut self, count: usize) -> usize {
    ((self.unit() * count as f32) as usize).min(count.max(1) - 1)
  }
}

#[derive(Default, Clone, Copy)]
pub struct Taps {
  pub low: f32,
  pub band: f32,
  pub high: f32
}

#[derive(Default, Clone, Copy)]
pub struct Svf {
  g: f32,
  k: f32,
  ic1: f32,
  ic2: f32
}

impl Svf {
  pub fn new(cutoff: f32, q: f32) -> Self {
    let mut filter = Self::default();
    filter.tune(cutoff, q);
    filter
  }

  pub fn tune(&mut self, cutoff: f32, q: f32) {
    self.g = (PI * cutoff.clamp(8.0, RATE * 0.45) / RATE).tan();
    self.k = 1.0 / q.max(0.3);
  }

  pub fn step(&mut self, x: f32) -> Taps {
    let &mut Svf { g, k, ic1, ic2 } = self;
    let v1 = (ic1 + g * (x - ic2)) / (1.0 + g * (g + k));
    let v2 = ic2 + g * v1;
    self.ic1 = 2.0 * v1 - ic1;
    self.ic2 = 2.0 * v2 - ic2;
    Taps { low: v2, band: v1 * k, high: x - k * v1 - v2 }
  }
}

#[derive(Default, Clone, Copy)]
pub struct Lag {
  y: f32,
  a: f32
}

impl Lag {
  pub fn new(cutoff: f32) -> Self {
    Self { y: 0.0, a: 1.0 - (-TAU * cutoff / RATE).exp() }
  }

  pub fn step(&mut self, x: f32) -> f32 {
    self.y += self.a * (x - self.y);
    self.y
  }
}

fn blep(t: f32, dt: f32) -> f32 {
  if t < dt {
    let x = t / dt;
    x + x - x * x - 1.0
  } else if t > 1.0 - dt {
    let x = (t - 1.0) / dt;
    x * x + x + x + 1.0
  } else {
    0.0
  }
}

#[derive(Default, Clone, Copy)]
pub struct Osc(pub f32);

impl Osc {
  fn advance(&mut self, freq: f32) { self.0 = (self.0 + freq / RATE).rem_euclid(1.0) }

  pub fn sine(&mut self, freq: f32) -> f32 {
    let out = (self.0 * TAU).sin();
    self.advance(freq);
    out
  }

  pub fn saw(&mut self, freq: f32) -> f32 {
    let out = 2.0 * self.0 - 1.0 - blep(self.0, (freq / RATE).clamp(1e-6, 0.5));
    self.advance(freq);
    out
  }
}

pub fn noise(seed: u64, count: usize) -> Vec<f32> {
  let mut rng = Rng::new(seed);
  (0..count).map(|_| rng.signed()).collect()
}

pub fn mix(into: &mut [f32], at: usize, source: &[f32], gain: f32) {
  let span = into.len().max(1);
  for (index, sample) in source.iter().enumerate() {
    into[(at + index) % span] += sample * gain
  }
}

pub fn seamless(raw: Vec<f32>, span: usize) -> Vec<f32> {
  let fade = raw.len().saturating_sub(span).min(span);
  (0..span)
    .map(|index| {
      (index < fade)
        .then(|| {
          let blend = index as f32 / fade as f32;
          raw[index] * blend.sqrt() + raw[span + index] * (1.0 - blend).sqrt()
        })
        .unwrap_or(raw[index])
    })
    .collect()
}

pub struct Wave(pub Vec<Vec<f32>>);

impl Wave {
  pub fn mono(samples: Vec<f32>) -> Self { Self(vec![samples]) }

  pub fn stereo(left: Vec<f32>, right: Vec<f32>) -> Self { Self(vec![left, right]) }

  pub fn peak(&self) -> f32 {
    self.0.iter().flatten().fold(0.0, |peak, sample| peak.max(sample.abs()))
  }

  #[cfg(test)]
  pub fn rms(&self) -> f32 {
    let count = self.0.iter().map(Vec::len).sum::<usize>().max(1);
    (self.0.iter().flatten().map(|sample| sample * sample).sum::<f32>() / count as f32)
      .sqrt()
  }

  pub fn normalized(self, target: f32) -> Self {
    let gain = target / self.peak().max(1e-9);
    Self(
      self
        .0
        .into_iter()
        .map(|channel| channel.into_iter().map(|x| x * gain).collect())
        .collect()
    )
  }

  pub fn trimmed(self) -> Self {
    let floor = self.peak() * 0.0005;
    let end = self
      .0
      .iter()
      .map(|channel| channel.iter().rposition(|x| x.abs() > floor).unwrap_or(0) + 1)
      .max()
      .unwrap_or(1);
    let fade = len(0.02).min(end);
    Self(
      self
        .0
        .into_iter()
        .map(|channel| {
          channel
            .into_iter()
            .take(end)
            .enumerate()
            .map(|(index, x)| x * ((end - index) as f32 / fade as f32).min(1.0))
            .collect()
        })
        .collect()
    )
  }

  #[cfg(test)]
  pub fn wav(&self) -> Vec<u8> {
    let channels = self.0.len() as u16;
    let frames = self.0.first().map(Vec::len).unwrap_or(0);
    let data = frames as u32 * channels as u32 * 2;
    let rate = RATE as u32;
    let header = [
      b"RIFF".to_vec(),
      (36 + data).to_le_bytes().to_vec(),
      b"WAVEfmt ".to_vec(),
      16u32.to_le_bytes().to_vec(),
      1u16.to_le_bytes().to_vec(),
      channels.to_le_bytes().to_vec(),
      rate.to_le_bytes().to_vec(),
      (rate * channels as u32 * 2).to_le_bytes().to_vec(),
      (channels * 2).to_le_bytes().to_vec(),
      16u16.to_le_bytes().to_vec(),
      b"data".to_vec(),
      data.to_le_bytes().to_vec()
    ]
    .concat();
    header
      .into_iter()
      .chain((0..frames).flat_map(|frame| {
        self.0.iter().flat_map(move |channel| {
          ((channel[frame].clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes()
        })
      }))
      .collect()
  }
}

struct Comb {
  line: Vec<f32>,
  at: usize,
  store: f32
}

impl Comb {
  fn step(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
    let out = self.line[self.at];
    self.store = out * (1.0 - damp) + self.store * damp;
    self.line[self.at] = x + self.store * feedback;
    self.at = (self.at + 1) % self.line.len();
    out
  }
}

struct Allpass {
  line: Vec<f32>,
  at: usize
}

impl Allpass {
  fn step(&mut self, x: f32) -> f32 {
    let held = self.line[self.at];
    self.line[self.at] = x + held * 0.5;
    self.at = (self.at + 1) % self.line.len();
    held - x
  }
}

pub struct Hall {
  combs: Vec<[Comb; 2]>,
  passes: Vec<[Allpass; 2]>,
  feedback: f32,
  damp: f32
}

impl Hall {
  const COMBS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
  const PASSES: [usize; 4] = [556, 441, 341, 225];
  const SPREAD: usize = 23;

  pub fn new(size: f32, damp: f32, stretch: f32) -> Self {
    let scaled = |base: usize, side: usize| {
      ((base + side * Self::SPREAD) as f32 * stretch * RATE / 44100.0) as usize
    };
    Self {
      combs: Self::COMBS
        .iter()
        .map(|&base| {
          [0, 1].map(|side| Comb {
            line: vec![0.0; scaled(base, side)],
            at: 0,
            store: 0.0
          })
        })
        .collect(),
      passes: Self::PASSES
        .iter()
        .map(|&base| {
          [0, 1].map(|side| Allpass { line: vec![0.0; scaled(base, side)], at: 0 })
        })
        .collect(),
      feedback: 0.7 + 0.28 * size,
      damp
    }
  }

  pub fn step(&mut self, x: f32) -> [f32; 2] {
    let Hall { combs, passes, feedback, damp } = self;
    let input = x * 0.015;
    [0, 1].map(|side| {
      let wet = combs
        .iter_mut()
        .map(|pair| pair[side].step(input, *feedback, *damp))
        .sum::<f32>();
      passes.iter_mut().fold(wet, |signal, pair| pair[side].step(signal))
    })
  }

  pub fn apply(mut self, dry: &Wave, wet: f32, circular: bool) -> Wave {
    let frames = dry.0[0].len();
    let channels = dry.0.len();
    let input = |frame: usize| {
      dry.0.iter().map(|channel| channel[frame]).sum::<f32>() / channels as f32
    };
    if circular {
      for frame in 0..frames {
        self.step(input(frame));
      }
    }
    let (left, right) = (0..frames)
      .map(|frame| {
        let [l, r] = self.step(input(frame));
        (dry.0[0][frame] + l * wet, dry.0[channels - 1][frame] + r * wet)
      })
      .unzip();
    Wave::stereo(left, right)
  }
}

#[derive(Clone, Copy)]
pub struct Phone {
  pub at: f32,
  pub pitch: f32,
  pub formants: [f32; 3],
  pub voice: f32,
  pub hiss: f32,
  pub trill: f32,
  pub breadth: f32
}

impl Phone {
  const BASE: Phone = Phone {
    at: 0.0,
    pitch: 110.0,
    formants: [500.0, 1500.0, 2500.0],
    voice: 1.0,
    hiss: 0.05,
    trill: 0.0,
    breadth: 1.0
  };
  pub const A: Phone = Phone { formants: [730.0, 1090.0, 2440.0], ..Self::BASE };
  pub const O: Phone = Phone { formants: [500.0, 850.0, 2500.0], ..Self::BASE };
  pub const U: Phone = Phone { formants: [320.0, 800.0, 2240.0], ..Self::BASE };
  pub const UH: Phone = Phone { formants: [640.0, 1190.0, 2390.0], ..Self::BASE };
  pub const EH: Phone = Phone { formants: [530.0, 1840.0, 2480.0], ..Self::BASE };
  pub const EE: Phone = Phone { formants: [290.0, 2250.0, 2900.0], ..Self::BASE };
  pub const R: Phone =
    Phone { formants: [450.0, 1300.0, 1700.0], trill: 0.9, ..Self::BASE };
  pub const S: Phone = Phone {
    formants: [4300.0, 6200.0, 8200.0],
    voice: 0.0,
    hiss: 0.5,
    breadth: 12.0,
    ..Self::BASE
  };
  pub const SH: Phone = Phone {
    formants: [2500.0, 3600.0, 5200.0],
    voice: 0.0,
    hiss: 0.5,
    breadth: 8.0,
    ..Self::BASE
  };
  pub const F: Phone = Phone {
    formants: [1600.0, 4200.0, 7600.0],
    voice: 0.0,
    hiss: 0.35,
    breadth: 25.0,
    ..Self::BASE
  };
  pub const D: Phone =
    Phone { formants: [400.0, 1700.0, 3200.0], voice: 0.15, hiss: 0.0, ..Self::BASE };
  pub const HUSH: Phone = Phone { voice: 0.0, hiss: 0.0, ..Self::BASE };

  pub fn at(self, at: f32) -> Self { Self { at, ..self } }

  pub fn pitched(self, pitch: f32) -> Self { Self { pitch, ..self } }

  pub fn voiced(self, voice: f32, hiss: f32) -> Self { Self { voice, hiss, ..self } }

  pub fn trilled(self, trill: f32) -> Self { Self { trill, ..self } }

  pub fn sized(self, scale: f32) -> Self {
    Self { formants: self.formants.map(|f| f * scale), ..self }
  }

  fn blend(self, other: Phone, amount: f32) -> Phone {
    let mix = |a: f32, b: f32| a + (b - a) * amount;
    Phone {
      at: mix(self.at, other.at),
      pitch: mix(self.pitch, other.pitch),
      formants: [0, 1, 2].map(|index| mix(self.formants[index], other.formants[index])),
      voice: mix(self.voice, other.voice),
      hiss: mix(self.hiss, other.hiss),
      trill: mix(self.trill, other.trill),
      breadth: mix(self.breadth, other.breadth)
    }
  }

  fn sample(phones: &[Phone], t: f32) -> Phone {
    phones
      .windows(2)
      .find(|pair| t < pair[1].at)
      .map(|pair| {
        pair[0].blend(
          pair[1],
          ((t - pair[0].at) / (pair[1].at - pair[0].at).max(1e-6)).clamp(0.0, 1.0)
        )
      })
      .unwrap_or(phones[phones.len() - 1])
  }
}

pub fn speak(phones: &[Phone], rough: f32, seed: u64) -> Vec<f32> {
  const WIDTHS: [f32; 3] = [90.0, 110.0, 160.0];
  const GAINS: [f32; 3] = [1.0, 0.7, 0.4];
  let mut rng = Rng::new(seed);
  let (mut glottis, mut under, mut flutter) =
    (Osc::default(), Osc::default(), Osc::default());
  let mut jitter = Lag::new(18.0);
  let mut shimmer = Lag::new(60.0);
  let mut bank = [Svf::default(); 3];
  let mut breath = Lag::new(5000.0);
  let end = phones.last().map(|phone| phone.at).unwrap_or(0.0);
  (0..len(end))
    .map(|index| {
      let phone = Phone::sample(phones, time(index));
      let wobble = 1.0 + rough * 0.05 * jitter.step(rng.signed() * 3.0);
      let pitch = phone.pitch * wobble;
      let rasp = 1.0 + rough * 0.8 * shimmer.step(rng.signed() * 2.0);
      let source = (glottis.saw(pitch) + rough * 0.7 * under.saw(pitch * 0.5)) * rasp;
      let trill = 1.0 - phone.trill * (0.5 + 0.5 * flutter.sine(26.0));
      let excite =
        source * phone.voice * trill + breath.step(rng.signed()) * phone.hiss * 2.0;
      bank
        .iter_mut()
        .zip(phone.formants)
        .zip(WIDTHS.iter().zip(GAINS))
        .map(|((filter, hz), (&width, gain))| {
          filter.tune(hz, hz / (width * phone.breadth));
          filter.step(excite).band * gain
        })
        .sum::<f32>()
    })
    .collect()
}
