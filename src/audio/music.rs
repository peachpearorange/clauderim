use super::synth::{Adsr, Hall, Lag, Osc, Phone, RATE, Rng, Svf, Wave, drive, len, midi,
                   mix, perc, smooth, time};

pub struct Saws {
  pub env: Adsr,
  pub voices: usize,
  pub detune: f32,
  pub floor: f32,
  pub tone: f32,
  pub vibrato: f32,
  pub scoop: f32,
  pub blat: f32
}

impl Saws {
  pub const STRINGS: Saws = Saws {
    env: Adsr { attack: 1.6, decay: 2.0, sustain: 0.85, release: 0.9 },
    voices: 6,
    detune: 0.0025,
    floor: 1.5,
    tone: 4.5,
    vibrato: 0.004,
    scoop: 0.0,
    blat: 0.0
  };
  pub const BASS: Saws = Saws {
    env: Adsr { attack: 1.2, decay: 2.0, sustain: 0.9, release: 0.9 },
    voices: 3,
    detune: 0.002,
    floor: 2.0,
    tone: 4.0,
    vibrato: 0.002,
    scoop: 0.0,
    blat: 0.0
  };
  pub const STACCATO: Saws = Saws {
    env: Adsr { attack: 0.01, decay: 0.1, sustain: 0.45, release: 0.05 },
    voices: 4,
    detune: 0.003,
    floor: 1.5,
    tone: 6.0,
    vibrato: 0.0,
    scoop: 0.0,
    blat: 1.0
  };
  pub const HORN: Saws = Saws {
    env: Adsr { attack: 0.12, decay: 0.8, sustain: 0.8, release: 0.3 },
    voices: 4,
    detune: 0.0018,
    floor: 0.9,
    tone: 3.2,
    vibrato: 0.003,
    scoop: 0.012,
    blat: 0.6
  };
  pub const BRASS: Saws = Saws {
    env: Adsr { attack: 0.02, decay: 0.25, sustain: 0.5, release: 0.15 },
    voices: 3,
    detune: 0.003,
    floor: 1.2,
    tone: 6.0,
    vibrato: 0.0,
    scoop: 0.008,
    blat: 1.8
  };

  pub fn play(&self, note: f32, held: f32, seed: u64) -> Vec<f32> {
    let &Saws { env, voices, detune, floor, tone, vibrato, scoop, blat } = self;
    let freq = midi(note);
    let mut rng = Rng::new(seed);
    let mut oscs: Vec<(Osc, f32)> = (0..voices)
      .map(|voice| {
        let spread = voice as f32 - (voices - 1) as f32 * 0.5;
        (Osc(rng.unit()), 1.0 + spread * detune + rng.signed() * detune * 0.3)
      })
      .collect();
    let mut wobble = Osc(rng.unit());
    let mut filter = Svf::default();
    let rate = rng.range(4.6, 5.6);
    (0..len(held + env.tail()))
      .map(|index| {
        let t = time(index);
        let level = env.level(t, held);
        let bend = (1.0 + vibrato * wobble.sine(rate) * smooth(t * 1.5 - 0.3))
          * (1.0 - scoop * (-t / 0.05).exp());
        if index % 32 == 0 {
          filter.tune(
            freq * (floor + tone * level * level * (1.0 + blat * perc(t, 0.01, 0.08))),
            0.8
          )
        }
        let raw =
          oscs.iter_mut().map(|(osc, ratio)| osc.saw(freq * *ratio * bend)).sum::<f32>();
        filter.step(raw / voices as f32).low * level
      })
      .collect()
  }
}

pub fn choir(notes: &[f32], held: f32, vowel: Phone, env: Adsr, seed: u64) -> Vec<f32> {
  const WIDTHS: [f32; 3] = [120.0, 150.0, 200.0];
  const GAINS: [f32; 3] = [1.0, 0.55, 0.3];
  let mut rng = Rng::new(seed);
  let mut singers: Vec<(Osc, Osc, f32, f32)> = notes
    .iter()
    .flat_map(|&note| (0..4).map(move |singer| (note, singer)))
    .map(|(note, singer)| {
      (
        Osc(rng.unit()),
        Osc(rng.unit()),
        midi(note) * (1.0 + (singer as f32 - 1.5) * 0.004),
        rng.range(4.8, 6.0)
      )
    })
    .collect();
  let mut bank = [0, 1, 2]
    .map(|index| Svf::new(vowel.formants[index], vowel.formants[index] / WIDTHS[index]));
  let mut body = Svf::new(900.0, 0.7);
  let mut breath = Lag::new(3000.0);
  let count = singers.len().max(1) as f32;
  (0..len(held + env.tail()))
    .map(|index| {
      let t = time(index);
      let level = env.level(t, held);
      let raw = singers
        .iter_mut()
        .map(|(osc, wobble, freq, rate)| {
          osc.saw(*freq * (1.0 + 0.004 * wobble.sine(*rate)))
        })
        .sum::<f32>()
        / count.sqrt()
        + breath.step(rng.signed()) * 0.3;
      let voiced = bank
        .iter_mut()
        .zip(GAINS)
        .map(|(filter, gain)| filter.step(raw).band * gain)
        .sum::<f32>();
      (voiced + body.step(raw).low * 0.25) * level
    })
    .collect()
}

pub fn flute(note: f32, held: f32, seed: u64) -> Vec<f32> {
  let env = Adsr { attack: 0.16, decay: 0.6, sustain: 0.85, release: 0.3 };
  let freq = midi(note);
  let mut rng = Rng::new(seed);
  let (mut osc, mut wobble) = (Osc::default(), Osc(rng.unit()));
  let mut breath = Svf::new(freq * 2.0, 1.5);
  (0..len(held + env.tail()))
    .map(|index| {
      let t = time(index);
      let level = env.level(t, held);
      let phase = osc.0 * std::f32::consts::TAU;
      osc.sine(freq * (1.0 + 0.006 * wobble.sine(5.0) * smooth(t * 2.0 - 0.6)));
      let tone = phase.sin() + 0.22 * (2.0 * phase).sin() + 0.07 * (3.0 * phase).sin();
      let chiff = perc(t, 0.01, 0.06);
      (tone + breath.step(rng.signed()).band * (0.12 + 0.8 * chiff)) * level
    })
    .collect()
}

pub fn drum(pitch: f32, decay: f32, seed: u64) -> Vec<f32> {
  let mut rng = Rng::new(seed);
  let (mut body, mut over, mut high) = (Osc::default(), Osc::default(), Osc::default());
  let mut skin = Svf::new(700.0, 0.7);
  (0..len(decay * 6.0))
    .map(|index| {
      let t = time(index);
      let freq = pitch * (1.0 + 0.7 * (-t / 0.025).exp());
      let tone = body.sine(freq) * perc(t, 0.002, decay)
        + over.sine(freq * 1.5) * perc(t, 0.002, decay * 0.45) * 0.45
        + high.sine(freq * 2.02) * perc(t, 0.002, decay * 0.25) * 0.2;
      drive(tone + skin.step(rng.signed()).low * perc(t, 0.001, 0.045) * 0.9, 1.6)
    })
    .collect()
}

pub fn harp(note: f32, seed: u64) -> Vec<f32> {
  let freq = midi(note);
  let mut rng = Rng::new(seed);
  let span = ((RATE / freq - 0.5).round() as usize).max(2);
  let ring = (4.0 - (note - 48.0) * 0.06).clamp(1.0, 4.5);
  let loss = 10f32.powf(-3.0 * span as f32 / (ring * RATE));
  let mut finger = Lag::new(freq * 5.0);
  let pluck: Vec<f32> = (0..span).map(|_| finger.step(rng.signed())).collect();
  let center = pluck.iter().sum::<f32>() / span as f32;
  let mut string: Vec<f32> = pluck.iter().map(|x| x - center).collect();
  let mut body = Svf::new(freq * 2.5, 0.6);
  (0..len(ring * 1.3))
    .map(|index| {
      let at = index % span;
      let out = string[at];
      string[at] = loss * 0.5 * (out + string[(at + 1) % span]);
      out + body.step(out).low * 0.4
    })
    .collect()
}

pub fn piano(note: f32, held: f32, seed: u64) -> Vec<f32> {
  let freq = midi(note);
  let mut rng = Rng::new(seed);
  let ring = (7.0 - (note - 60.0) * 0.1).clamp(1.5, 9.0);
  let mut partials: Vec<(Osc, Osc, f32, f32, f32)> = (1..=10)
    .map(|harmonic| harmonic as f32)
    .map(|n| {
      (
        Osc(rng.unit()),
        Osc(rng.unit()),
        freq * n * (1.0 + 0.0004 * n * n).sqrt(),
        1.0 / n.powf(1.4),
        ring / (1.0 + 0.6 * (n - 1.0))
      )
    })
    .filter(|&(_, _, hz, _, _)| hz < RATE * 0.45)
    .collect();
  let mut hammer = Svf::new(freq * 4.0, 0.7);
  (0..len(held.min(ring * 1.5) + 0.6))
    .map(|index| {
      let t = time(index);
      let damper = (t < held).then_some(1.0).unwrap_or((-(t - held) / 0.12).exp());
      let tone = partials
        .iter_mut()
        .map(|(one, two, hz, gain, decay)| {
          (one.sine(*hz) + two.sine(*hz * 1.0008)) * 0.5 * *gain * perc(t, 0.002, *decay)
        })
        .sum::<f32>();
      (tone + hammer.step(rng.signed()).band * perc(t, 0.001, 0.012) * 0.4) * damper
    })
    .collect()
}

pub fn timpani(note: f32, seed: u64) -> Vec<f32> {
  const MODES: [(f32, f32, f32); 5] = [
    (1.0, 1.0, 1.0),
    (1.5, 0.55, 0.7),
    (1.99, 0.35, 0.5),
    (2.44, 0.2, 0.35),
    (2.97, 0.12, 0.25)
  ];
  let freq = midi(note);
  let mut rng = Rng::new(seed);
  let mut heads = MODES.map(|_| Osc(rng.unit()));
  let mut mallet = Svf::new(350.0, 0.7);
  (0..len(4.0))
    .map(|index| {
      let t = time(index);
      let glide = 1.0 + 0.015 * (-t / 0.06).exp();
      let tone = heads
        .iter_mut()
        .zip(MODES)
        .map(|(head, (ratio, gain, decay))| {
          head.sine(freq * ratio * glide) * gain * perc(t, 0.003, decay * 1.4)
        })
        .sum::<f32>();
      tone + mallet.step(rng.signed()).low * perc(t, 0.001, 0.03) * 1.2
    })
    .collect()
}

pub fn cymbal(swell: f32, seed: u64) -> Vec<f32> {
  const SHIMMER: [(f32, f32); 3] = [(4800.0, 1.0), (7600.0, 0.8), (11000.0, 0.5)];
  let mut rng = Rng::new(seed);
  let mut bands = SHIMMER.map(|(hz, _)| Svf::new(hz, 1.2));
  (0..len(swell + 3.0))
    .map(|index| {
      let t = time(index);
      let level =
        (t < swell).then(|| (t / swell).powi(3)).unwrap_or((-(t - swell) / 0.7).exp());
      let hiss = rng.signed();
      bands
        .iter_mut()
        .zip(SHIMMER)
        .map(|(band, (_, gain))| band.step(hiss).band * gain)
        .sum::<f32>()
        * level
    })
    .collect()
}

struct Score {
  left: Vec<f32>,
  right: Vec<f32>,
  rng: Rng
}

impl Score {
  fn new(secs: f32, seed: u64) -> Self {
    Self { left: vec![0.0; len(secs)], right: vec![0.0; len(secs)], rng: Rng::new(seed) }
  }

  fn seed(&mut self) -> u64 { (self.rng.unit() * 1e9) as u64 }

  fn place(&mut self, at: f32, sound: &[f32], gain: f32, pan: f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    let start = (at * RATE) as usize;
    mix(&mut self.left, start, sound, gain * angle.cos());
    mix(&mut self.right, start, sound, gain * angle.sin())
  }

  fn finish(self, hall: Hall, wet: f32) -> Wave {
    hall.apply(&Wave::stereo(self.left, self.right), wet, true).normalized(0.9)
  }
}

pub fn explore() -> Wave {
  const CHORD: f32 = 8.0;
  const CHORDS: [(f32, [f32; 3]); 12] = [
    (38.0, [50.0, 57.0, 65.0]),
    (34.0, [46.0, 53.0, 62.0]),
    (41.0, [53.0, 57.0, 60.0]),
    (36.0, [48.0, 55.0, 64.0]),
    (38.0, [50.0, 57.0, 62.0]),
    (43.0, [50.0, 55.0, 58.0]),
    (34.0, [50.0, 53.0, 58.0]),
    (45.0, [52.0, 57.0, 61.0]),
    (38.0, [50.0, 57.0, 65.0]),
    (34.0, [46.0, 53.0, 62.0]),
    (31.0, [50.0, 55.0, 58.0]),
    (33.0, [52.0, 57.0, 62.0])
  ];
  const HORN_CALL: [(f32, f32, f32); 11] = [
    (0.0, 1.5, 69.0),
    (1.5, 1.0, 72.0),
    (2.5, 0.5, 69.0),
    (3.0, 1.0, 67.0),
    (4.0, 2.0, 65.0),
    (6.5, 1.5, 64.0),
    (8.0, 1.5, 67.0),
    (9.5, 1.0, 64.0),
    (10.5, 1.0, 62.0),
    (11.5, 0.5, 60.0),
    (12.0, 3.5, 62.0)
  ];
  const FLUTE_SONG: [(f32, f32, f32); 16] = [
    (0.0, 2.0, 74.0),
    (2.0, 0.5, 72.0),
    (2.5, 0.5, 70.0),
    (3.0, 1.5, 69.0),
    (4.5, 1.0, 67.0),
    (5.5, 2.5, 69.0),
    (8.0, 1.0, 65.0),
    (9.0, 1.0, 67.0),
    (10.0, 1.5, 70.0),
    (11.5, 0.5, 69.0),
    (12.0, 2.0, 67.0),
    (14.0, 1.5, 65.0),
    (16.0, 1.5, 64.0),
    (17.5, 0.5, 62.0),
    (18.0, 1.5, 61.0),
    (19.5, 2.5, 64.0)
  ];
  const LOW_HORN: [(f32, f32, f32); 11] = [
    (0.0, 2.0, 53.0),
    (2.0, 1.0, 57.0),
    (3.0, 1.0, 58.0),
    (4.0, 2.5, 57.0),
    (8.0, 1.5, 55.0),
    (9.5, 1.0, 58.0),
    (10.5, 2.5, 62.0),
    (13.0, 1.0, 60.0),
    (16.0, 3.0, 57.0),
    (19.0, 1.0, 55.0),
    (20.0, 3.5, 57.0)
  ];
  const TIMPANI: [(f32, f32, f32); 5] = [
    (0.0, 38.0, 1.0),
    (32.0, 38.0, 0.9),
    (48.0, 33.0, 0.5),
    (64.0, 38.0, 0.9),
    (95.0, 33.0, 0.6)
  ];
  const ROLLS: [(f32, f32, f32); 3] =
    [(29.5, 2.5, 33.0), (61.5, 2.5, 33.0), (93.0, 2.0, 38.0)];
  const HARP_STEPS: [usize; 8] = [0, 1, 2, 3, 4, 3, 2, 1];
  const PIANO_STEPS: [(f32, usize); 3] = [(0.0, 2), (2.5, 1), (5.0, 0)];
  let mut score = Score::new(CHORD * CHORDS.len() as f32, 7);
  for (index, &(bass, pad)) in CHORDS.iter().enumerate() {
    let at = index as f32 * CHORD;
    let seed = score.seed();
    score.place(at, &Saws::BASS.play(bass, CHORD, seed), 0.3, 0.0);
    for (voice, &note) in pad.iter().enumerate() {
      let seed = score.seed();
      score.place(
        at,
        &Saws::STRINGS.play(note, CHORD - 0.3, seed),
        0.3,
        voice as f32 * 0.6 - 0.6
      )
    }
    if (4..12).contains(&index) {
      let seed = score.seed();
      let vowel = [Phone::A, Phone::O][index % 2];
      let env = Adsr { attack: 2.5, decay: 3.0, sustain: 0.8, release: 1.2 };
      score.place(
        at,
        &choir(&[pad[0] - 12.0, pad[0], pad[1]], CHORD - 0.5, vowel, env, seed),
        0.35,
        0.0
      )
    }
    if index < 4 {
      for &(beat, voice) in PIANO_STEPS.iter() {
        let seed = score.seed();
        score.place(at + beat, &piano(pad[voice] + 12.0, 2.5, seed), 0.22, 0.15)
      }
    } else {
      let strings = [bass + 12.0, pad[0], pad[1], pad[2], pad[0] + 12.0];
      for step in 0..16 {
        let seed = score.seed();
        score.place(
          at + step as f32 * 0.5,
          &harp(strings[HARP_STEPS[step % HARP_STEPS.len()]], seed),
          0.14,
          0.4
        )
      }
    }
  }
  for (start, line, shift) in
    [(16.0, &HORN_CALL[..], 0.0), (72.0, &LOW_HORN[..], 0.0)].into_iter()
  {
    for &(at, held, note) in line.iter() {
      let seed = score.seed();
      score.place(start + at, &Saws::HORN.play(note + shift, held, seed), 0.32, -0.25)
    }
  }
  for &(at, held, note) in FLUTE_SONG.iter() {
    let seed = score.seed();
    score.place(40.0 + at, &flute(note, held, seed), 0.2, 0.3)
  }
  for &(at, note, gain) in TIMPANI.iter() {
    let seed = score.seed();
    score.place(at, &timpani(note, seed), gain * 0.3, 0.1)
  }
  for &(start, secs, note) in ROLLS.iter() {
    for stroke in 0..(secs / 0.07) as usize {
      let seed = score.seed();
      let at = stroke as f32 * 0.07;
      score.place(
        start + at,
        &timpani(note, seed),
        0.02 + 0.08 * (at / secs).powi(2),
        0.1
      )
    }
  }
  score.finish(Hall::new(0.9, 0.4, 1.35), 1.1)
}

pub fn combat() -> Wave {
  const BEAT: f32 = 0.5;
  const CHORDS: [(f32, f32); 8] = [
    (38.0, 3.0),
    (38.0, 3.0),
    (34.0, 4.0),
    (36.0, 4.0),
    (38.0, 3.0),
    (38.0, 3.0),
    (43.0, 3.0),
    (45.0, 4.0)
  ];
  const MELODY: [(f32, f32); 30] = [
    (62.0, 2.0),
    (69.0, 2.0),
    (67.0, 1.0),
    (65.0, 1.0),
    (64.0, 2.0),
    (65.0, 3.0),
    (67.0, 1.0),
    (69.0, 4.0),
    (70.0, 2.0),
    (69.0, 2.0),
    (65.0, 2.0),
    (62.0, 2.0),
    (64.0, 2.0),
    (67.0, 2.0),
    (72.0, 2.0),
    (70.0, 1.0),
    (69.0, 1.0),
    (69.0, 6.0),
    (0.0, 2.0),
    (74.0, 2.0),
    (72.0, 2.0),
    (69.0, 4.0),
    (70.0, 2.0),
    (69.0, 2.0),
    (67.0, 2.0),
    (62.0, 2.0),
    (64.0, 4.0),
    (73.0, 2.0),
    (69.0, 2.0),
    (0.0, 0.0)
  ];
  const BIG: [f32; 5] = [0.0, 1.5, 2.0, 3.0, 3.5];
  const SMALL: [f32; 3] = [0.5, 1.0, 2.5];
  let bars = 32;
  let mut score = Score::new(bars as f32 * 4.0 * BEAT, 11);
  for bar in 0..bars {
    let bar_at = bar as f32 * 4.0 * BEAT;
    let (root, third) = CHORDS[(bar / 2) % CHORDS.len()];
    let pattern = [0.0, 0.0, 12.0, 0.0, third, 0.0, 7.0, 0.0];
    for (step, &offset) in pattern.iter().enumerate() {
      let seed = score.seed();
      let gain = if step % 4 == 0 { 0.6 } else { 0.45 };
      score.place(
        bar_at + step as f32 * BEAT * 0.5,
        &Saws::STACCATO.play(root + 12.0 + offset, BEAT * 0.3, seed),
        gain,
        -0.3
      )
    }
    for &beat in BIG.iter() {
      let seed = score.seed();
      score.place(
        bar_at + beat * BEAT,
        &drum(48.0, 0.45, seed),
        if beat == 0.0 { 0.55 } else { 0.38 },
        0.05
      )
    }
    for (beat, note, gain) in [(0.0, root, 0.4), (2.0, root + 7.0, 0.28)] {
      let seed = score.seed();
      score.place(bar_at + beat * BEAT, &timpani(note.max(33.0), seed), gain, -0.1)
    }
    for &beat in SMALL.iter() {
      let seed = score.seed();
      score.place(bar_at + beat * BEAT, &drum(110.0, 0.14, seed), 0.2, -0.2)
    }
    if bar % 8 == 6 {
      let seed = score.seed();
      score.place(bar_at, &cymbal(8.0 * BEAT, seed), 0.12, 0.3)
    }
    if bar >= 8 && bar % 4 == 0 {
      let seed = score.seed();
      let env = Adsr { attack: 0.6, decay: 2.0, sustain: 0.85, release: 0.8 };
      score.place(
        bar_at,
        &choir(
          &[root + 12.0, root + 19.0, root + 24.0],
          15.0 * BEAT,
          [Phone::O, Phone::A][bar / 4 % 2],
          env,
          seed
        ),
        0.3,
        0.0
      )
    }
    if bar % 8 == 7 {
      for tick in 0..4 {
        let seed = score.seed();
        score.place(
          bar_at + (3.0 + tick as f32 * 0.25) * BEAT,
          &drum(88.0, 0.12, seed),
          0.15 + tick as f32 * 0.05,
          0.2
        )
      }
    }
    if bar % 2 == 0 {
      let chord = [root, root + 7.0, root + 12.0, root + 12.0 + third];
      for (beat, held) in [(0.0, 0.8), (2.5, 0.35), (6.5, 0.5)] {
        for &note in chord.iter() {
          let seed = score.seed();
          score.place(
            bar_at + beat * BEAT,
            &Saws::BRASS.play(note, held * BEAT, seed),
            0.16,
            0.25
          )
        }
      }
      let seed = score.seed();
      score.place(bar_at, &Saws::BASS.play(root, 8.0 * BEAT, seed), 0.2, 0.0)
    }
  }
  MELODY.iter().fold(64.0 * BEAT, |at, &(note, beats)| {
    if note > 0.0 {
      for (octave, gain) in [(0.0, 0.3), (-12.0, 0.22)] {
        let seed = score.seed();
        score.place(
          at,
          &Saws::HORN.play(note + octave, beats * BEAT * 0.92, seed),
          gain,
          -0.1
        )
      }
    }
    at + beats * BEAT
  });
  score.finish(Hall::new(0.7, 0.45, 1.1), 0.6)
}

pub fn swell(notes: &[f32], held: f32, vowel: Phone, seed: u64) -> Vec<f32> {
  choir(
    notes,
    held,
    vowel,
    Adsr { attack: held * 0.6, decay: 2.0, sustain: 0.9, release: 1.0 },
    seed
  )
}
