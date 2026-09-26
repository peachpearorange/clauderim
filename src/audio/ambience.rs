use {super::{music::choir,
             synth::{Adsr, Hall, Lag, Osc, Phone, Rng, Svf, Wave, curve, len, mix,
                     seamless, speak, time}},
     std::f32::consts::TAU};

fn gust(t: f32, span: f32, seed: u64) -> f32 {
  let mut rng = Rng::new(seed);
  [1.0f32, 2.0, 3.0, 5.0, 8.0, 13.0]
    .iter()
    .map(|&cycles| (t / span * cycles * TAU + rng.unit() * TAU).sin() / cycles.sqrt())
    .sum::<f32>()
    * 0.3
    + 0.5
}

fn tone(span: f32, hz: f32) -> f32 { (hz * span).round().max(1.0) / span }

pub fn wind() -> Wave {
  const SPAN: f32 = 32.0;
  let channel = |seed: u64| {
    let mut rng = Rng::new(seed);
    let (mut body, mut whistle, mut rumble) =
      (Svf::default(), Svf::default(), Lag::new(70.0));
    seamless(
      (0..len(SPAN + 2.0))
        .map(|index| {
          let t = time(index);
          let blow = gust(t, SPAN, 3).clamp(0.05, 1.0) * 0.85
            + gust(t, SPAN, seed).clamp(0.0, 1.0) * 0.15;
          if index % 16 == 0 {
            body.tune(140.0 + 380.0 * blow * blow, 0.5);
            whistle.tune(380.0 + 260.0 * blow, 2.5)
          }
          let x = rng.signed();
          body.step(x).low * (0.3 + 0.7 * blow)
            + whistle.step(x).band * blow.powi(3) * 0.12
            + rumble.step(x) * 2.0 * blow
        })
        .collect(),
      len(SPAN)
    )
  };
  Wave::stereo(channel(21), channel(22)).normalized(0.9)
}

pub fn night() -> Wave {
  const SPAN: f32 = 24.0;
  let mut rng = Rng::new(31);
  let mut left = vec![0.0; len(SPAN)];
  let mut right = vec![0.0; len(SPAN)];
  for _ in 0..7 {
    let hz = rng.range(3800.0, 5200.0);
    let pulses = 3 + rng.below(2);
    let period = rng.range(0.45, 0.9);
    let (pan, gain) = (rng.unit(), rng.range(0.25, 1.0));
    let mut osc = Osc::default();
    let chirp: Vec<f32> = (0..len(pulses as f32 * 0.028))
      .map(|index| {
        let within = (time(index) / 0.028).fract() / 0.5;
        osc.sine(hz)
          * if within < 1.0 { (within * std::f32::consts::PI).sin().powi(2) } else { 0.0 }
      })
      .collect();
    let offset = rng.range(0.0, period);
    for beat in 0..(SPAN / period) as usize {
      if rng.unit() < 0.85 {
        let at = len(offset + beat as f32 * period + rng.range(-0.02, 0.02));
        let level = gain * rng.range(0.8, 1.0);
        mix(&mut left, at, &chirp, level * (1.0 - pan));
        mix(&mut right, at, &chirp, level * pan)
      }
    }
  }
  let mut chorus: Vec<(Osc, f32, f32, f32)> = (0..6)
    .map(|_| {
      (
        Osc(rng.unit()),
        tone(SPAN, rng.range(4200.0, 4700.0)),
        tone(SPAN, rng.range(14.0, 22.0)),
        rng.unit()
      )
    })
    .collect();
  for (index, (l, r)) in left.iter_mut().zip(right.iter_mut()).enumerate() {
    let t = time(index);
    let hum = chorus
      .iter_mut()
      .map(|(osc, hz, rate, phase)| {
        osc.sine(*hz) * (0.5 + 0.5 * ((t * *rate + *phase) * TAU).sin()).powi(4)
      })
      .sum::<f32>()
      * 0.05
      * gust(t, SPAN, 5);
    *l += hum;
    *r += hum
  }
  Hall::new(0.5, 0.5, 0.8).apply(&Wave::stereo(left, right), 0.3, true).normalized(0.9)
}

pub fn cave() -> Wave {
  const SPAN: f32 = 40.0;
  let mut rng = Rng::new(41);
  let mut drones: Vec<(Osc, f32, f32)> =
    [(55.0, 0.5), (55.25, 0.4), (82.5, 0.25), (110.1, 0.12)]
      .map(|(hz, gain)| (Osc(rng.unit()), tone(SPAN, hz), gain))
      .to_vec();
  let (mut rumble, mut moan, mut noise) = (Lag::new(110.0), Svf::default(), Rng::new(42));
  let base: Vec<f32> = seamless(
    (0..len(SPAN + 2.0))
      .map(|index| {
        let t = time(index);
        let breathe = gust(t, SPAN, 7);
        let x = noise.signed();
        if index % 16 == 0 {
          moan.tune(300.0 + 140.0 * gust(t, SPAN, 8), 7.0)
        }
        drones.iter_mut().map(|(osc, hz, gain)| osc.sine(*hz) * *gain).sum::<f32>()
          * (0.6 + 0.4 * breathe)
          * 0.4
          + rumble.step(x) * 2.0
          + moan.step(x).band * breathe.powi(2) * 0.6
      })
      .collect(),
    len(SPAN)
  );
  let mut left = base.clone();
  let mut right = base;
  for _ in 0..16 {
    let at = len(rng.range(0.0, SPAN));
    let hz = rng.range(700.0, 1700.0);
    let mut osc = Osc::default();
    let drip: Vec<f32> = (0..len(0.08))
      .map(|index| {
        let t = time(index);
        osc.sine(hz * (1.0 + 2.5 * (t / 0.04).min(1.0)))
          * (t / 0.001).min(1.0)
          * (-t / 0.02).exp()
      })
      .collect();
    let (pan, gain) = (rng.unit(), rng.range(0.1, 0.35));
    mix(&mut left, at, &drip, gain * (1.0 - pan));
    mix(&mut right, at, &drip, gain * pan)
  }
  Hall::new(0.93, 0.25, 1.6).apply(&Wave::stereo(left, right), 1.2, true).normalized(0.9)
}

pub fn chant(secs: f32, looping: bool, seed: u64) -> Wave {
  const VOWELS: [Phone; 5] = [Phone::A, Phone::O, Phone::U, Phone::EH, Phone::EE];
  const CONSONANTS: [Phone; 4] = [Phone::S, Phone::SH, Phone::F, Phone::HUSH];
  let mut rng = Rng::new(seed);
  let whisper = |rng: &mut Rng| {
    let mut phones = vec![Phone::HUSH.at(0.0)];
    let mut at = rng.range(0.2, 1.2);
    while at < secs - 2.0 {
      for _ in 0..4 + rng.below(4) {
        let consonant = CONSONANTS[rng.below(CONSONANTS.len())];
        let vowel = VOWELS[rng.below(VOWELS.len())].voiced(0.0, 0.5);
        let length = rng.range(0.22, 0.38);
        phones.extend([
          consonant.voiced(0.0, consonant.hiss * 0.8).at(at),
          vowel.at(at + 0.07),
          vowel.at(at + length - 0.06),
          Phone::HUSH.at(at + length)
        ]);
        at += length + rng.range(0.0, 0.05)
      }
      at += rng.range(0.8, 2.0)
    }
    phones.push(Phone::HUSH.at(at));
    speak(&phones, 0.0, (rng.unit() * 1e6) as u64)
  };
  let singer = |rng: &mut Rng| {
    let steps = (secs / 1.6) as usize;
    let mut phones: Vec<Phone> = (0..steps)
      .map(|step| {
        VOWELS[rng.below(3)]
          .pitched(if step % 4 == 3 { 110.0 } else { 146.8 })
          .voiced(0.8, 0.3)
          .at(step as f32 * 1.6 + 0.4)
      })
      .collect();
    phones.insert(0, Phone::HUSH.pitched(146.8).at(0.0));
    phones.push(Phone::HUSH.pitched(146.8).at(secs));
    speak(&phones, 0.2, (rng.unit() * 1e6) as u64)
  };
  let hum = Adsr {
    attack: if looping { 0.01 } else { 2.0 },
    decay: 3.0,
    sustain: 1.0,
    release: 1.5
  };
  let drone = choir(
    &[38.0, 45.0, 50.0],
    secs + if looping { 2.0 } else { -1.5 },
    Phone::U,
    hum,
    seed
  );
  let drone = if looping { seamless(drone, len(secs)) } else { drone };
  let total = if looping { secs } else { secs + 3.5 };
  let mut left = vec![0.0; len(total)];
  let mut right = vec![0.0; len(total)];
  mix(&mut left, 0, &drone, 0.5);
  mix(&mut right, 0, &drone, 0.5);
  for &pan in [-0.8f32, 0.0, 0.8].iter() {
    let voice = whisper(&mut rng);
    mix(&mut left, 0, &voice, 0.6 * (1.0 - pan) * 0.5 + 0.1);
    mix(&mut right, 0, &voice, 0.6 * (1.0 + pan) * 0.5 + 0.1)
  }
  let sung = singer(&mut rng);
  mix(&mut left, 0, &sung, 0.12);
  mix(&mut right, 0, &sung, 0.12);
  let shape = |t: f32| {
    if looping {
      1.0
    } else {
      curve(t, &[(0.0, 0.0), (1.0, 1.0), (secs - 1.5, 1.0), (secs, 0.0)])
    }
  };
  let dry = Wave(
    [left, right]
      .into_iter()
      .map(|channel| {
        channel.into_iter().enumerate().map(|(index, x)| x * shape(time(index))).collect()
      })
      .collect()
  );
  Hall::new(0.92, 0.35, 1.4).apply(&dry, 1.0, looping).normalized(0.9)
}

pub fn bird(seed: u64) -> Wave {
  let mut rng = Rng::new(seed);
  let trill = seed % 3 == 0;
  let notes = if trill { 6 + rng.below(6) } else { 3 + rng.below(5) };
  let (low, high) = (rng.range(2500.0, 4000.0), rng.range(4000.0, 6500.0));
  let warble = rng.range(0.0, 0.04);
  let mut song = vec![0.0; len(2.5)];
  let (repeat_from, repeat_to) = (rng.range(low, high), rng.range(low, high));
  (0..notes).fold(0.05, |at, _| {
    let length = if trill { 0.035 } else { rng.range(0.05, 0.14) };
    let (from, to) = if trill {
      (repeat_from, repeat_to)
    } else {
      (rng.range(low, high), rng.range(low, high))
    };
    let mut osc = Osc::default();
    let mut flutter = Osc::default();
    let note: Vec<f32> = (0..len(length))
      .map(|index| {
        let u = time(index) / length;
        let hz = (from + (to - from) * u) * (1.0 + warble * flutter.sine(45.0));
        osc.sine(hz) * (u * std::f32::consts::PI).sin().powi(2)
      })
      .collect();
    let gain = rng.range(0.5, 1.0);
    mix(&mut song, len(at), &note, gain);
    at + length + if trill { 0.02 } else { rng.range(0.02, 0.09) }
  });
  Wave::mono(song).trimmed().normalized(0.9)
}

pub fn owl(seed: u64) -> Wave {
  let mut rng = Rng::new(seed);
  let pitch = rng.range(360.0, 420.0);
  let (mut osc, mut breath) = (Osc::default(), Lag::new(800.0));
  let hoots = [(0.0, 0.45), (0.75, 0.2), (1.0, 0.5)];
  Wave::mono(
    (0..len(1.7))
      .map(|index| {
        let t = time(index);
        let level = hoots
          .iter()
          .map(|&(at, length)| {
            let u = (t - at) / length;
            if (0.0..1.0).contains(&u) {
              (u * std::f32::consts::PI).sin().powf(1.5)
            } else {
              0.0
            }
          })
          .sum::<f32>();
        let phase = osc.0 * TAU;
        osc.sine(pitch * (1.0 - 0.06 * (t * 2.0).fract()));
        (phase.sin() + 0.15 * (2.0 * phase).sin() + breath.step(rng.signed()) * 0.8)
          * level
      })
      .collect()
  )
  .normalized(0.9)
}
