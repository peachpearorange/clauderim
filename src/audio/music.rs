use super::synth::{Adsr, Hall, Lag, Osc, Phone, RATE, Rng, Svf, Wave, drive, len, midi,
                   mix, perc, smooth, time};

pub struct Saws {
  pub env: Adsr,
  pub voices: usize,
  pub detune: f32,
  pub floor: f32,
  pub tone: f32,
  pub vibrato: f32
}

impl Saws {
  pub const STRINGS: Saws = Saws {
    env: Adsr { attack: 1.6, decay: 2.0, sustain: 0.85, release: 0.9 },
    voices: 3,
    detune: 0.0035,
    floor: 1.5,
    tone: 5.0,
    vibrato: 0.003
  };
  pub const BASS: Saws = Saws {
    env: Adsr { attack: 1.2, decay: 2.0, sustain: 0.9, release: 0.9 },
    voices: 2,
    detune: 0.002,
    floor: 2.0,
    tone: 4.0,
    vibrato: 0.0
  };
  pub const STACCATO: Saws = Saws {
    env: Adsr { attack: 0.012, decay: 0.12, sustain: 0.55, release: 0.05 },
    voices: 2,
    detune: 0.003,
    floor: 2.0,
    tone: 7.0,
    vibrato: 0.0
  };
  pub const HORN: Saws = Saws {
    env: Adsr { attack: 0.14, decay: 0.8, sustain: 0.8, release: 0.3 },
    voices: 2,
    detune: 0.0015,
    floor: 1.2,
    tone: 5.5,
    vibrato: 0.004
  };
  pub const BRASS: Saws = Saws {
    env: Adsr { attack: 0.02, decay: 0.2, sustain: 0.4, release: 0.12 },
    voices: 3,
    detune: 0.004,
    floor: 1.5,
    tone: 9.0,
    vibrato: 0.0
  };

  pub fn play(&self, note: f32, held: f32, seed: u64) -> Vec<f32> {
    let &Saws { env, voices, detune, floor, tone, vibrato } = self;
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
        let bend = 1.0 + vibrato * wobble.sine(rate) * smooth(t * 1.5 - 0.3);
        if index % 32 == 0 {
          filter.tune(freq * (floor + tone * level * level), 0.8)
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
  const DRUMS: [(f32, f32); 11] = [
    (0.0, 1.0),
    (30.5, 0.5),
    (31.25, 0.6),
    (32.0, 0.9),
    (62.5, 0.5),
    (63.25, 0.6),
    (64.0, 0.9),
    (93.5, 0.4),
    (94.25, 0.5),
    (95.0, 0.6),
    (48.0, 0.5)
  ];
  let mut score = Score::new(CHORD * CHORDS.len() as f32, 7);
  CHORDS.iter().enumerate().for_each(|(index, &(bass, pad))| {
    let at = index as f32 * CHORD;
    let seed = score.seed();
    score.place(at, &Saws::BASS.play(bass, CHORD, seed), 0.3, 0.0);
    pad.iter().enumerate().for_each(|(voice, &note)| {
      let seed = score.seed();
      score.place(
        at,
        &Saws::STRINGS.play(note, CHORD - 0.3, seed),
        0.3,
        voice as f32 * 0.6 - 0.6
      )
    });
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
  });
  [(16.0, &HORN_CALL[..], 0.0), (72.0, &LOW_HORN[..], 0.0)].into_iter().for_each(
    |(start, line, shift)| {
      line.iter().for_each(|&(at, held, note)| {
        let seed = score.seed();
        score.place(start + at, &Saws::HORN.play(note + shift, held, seed), 0.32, -0.25)
      })
    }
  );
  FLUTE_SONG.iter().for_each(|&(at, held, note)| {
    let seed = score.seed();
    score.place(40.0 + at, &flute(note, held, seed), 0.2, 0.3)
  });
  DRUMS.iter().for_each(|&(at, gain)| {
    let seed = score.seed();
    score.place(at, &drum(midi(38.0) * 0.98, 0.5, seed), gain * 0.35, 0.1)
  });
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
  (0..bars).for_each(|bar| {
    let bar_at = bar as f32 * 4.0 * BEAT;
    let (root, third) = CHORDS[(bar / 2) % CHORDS.len()];
    let pattern = [0.0, 0.0, 12.0, 0.0, third, 0.0, 7.0, 0.0];
    pattern.iter().enumerate().for_each(|(step, &offset)| {
      let seed = score.seed();
      let gain = if step % 4 == 0 { 0.6 } else { 0.45 };
      score.place(
        bar_at + step as f32 * BEAT * 0.5,
        &Saws::STACCATO.play(root + 12.0 + offset, BEAT * 0.3, seed),
        gain,
        -0.3
      )
    });
    BIG.iter().for_each(|&beat| {
      let seed = score.seed();
      score.place(
        bar_at + beat * BEAT,
        &drum(58.0, 0.3, seed),
        if beat == 0.0 { 0.5 } else { 0.35 },
        0.05
      )
    });
    SMALL.iter().for_each(|&beat| {
      let seed = score.seed();
      score.place(bar_at + beat * BEAT, &drum(110.0, 0.14, seed), 0.2, -0.2)
    });
    if bar % 8 == 7 {
      (0..4).for_each(|tick| {
        let seed = score.seed();
        score.place(
          bar_at + (3.0 + tick as f32 * 0.25) * BEAT,
          &drum(88.0, 0.12, seed),
          0.15 + tick as f32 * 0.05,
          0.2
        )
      })
    }
    if bar % 2 == 0 {
      let chord = [root + 12.0, root + 12.0 + third, root + 19.0, root + 24.0];
      [(0.0, 0.8), (2.5, 0.35), (6.5, 0.5)].into_iter().for_each(|(beat, held)| {
        chord.iter().for_each(|&note| {
          let seed = score.seed();
          score.place(
            bar_at + beat * BEAT,
            &Saws::BRASS.play(note, held * BEAT, seed),
            0.16,
            0.25
          )
        })
      });
      let seed = score.seed();
      score.place(bar_at, &Saws::BASS.play(root, 8.0 * BEAT, seed), 0.2, 0.0)
    }
  });
  MELODY.iter().fold(64.0 * BEAT, |at, &(note, beats)| {
    if note > 0.0 {
      let seed = score.seed();
      score.place(at, &Saws::HORN.play(note, beats * BEAT * 0.92, seed), 0.3, -0.1)
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
