use {super::{ambience::chant,
             music::{Saws, choir, drum, swell},
             synth::{Adsr, Hall, Lag, Osc, Phone, RATE, Rng, Svf, Wave, curve, drive,
                     len, midi, mix, noise, perc, speak, time}},
     crate::signal::Cue};

fn whoosh(seed: u64, dur: f32, pitch: f32, heft: f32) -> Vec<f32> {
  let mut rng = Rng::new(seed);
  let crest = rng.range(0.38, 0.5) * dur;
  let top = rng.range(1500.0, 2100.0) * pitch;
  let (mut air, mut body) = (Svf::default(), Lag::new(220.0));
  noise(seed, len(dur + 0.05))
    .into_iter()
    .enumerate()
    .map(|(index, x)| {
      let t = time(index);
      let rise = curve(t, &[(0.0, 0.0), (crest, 1.0), (dur, 0.0)]);
      air.tune(220.0 * pitch + top * rise * rise, 2.0);
      (air.step(x).band + body.step(x) * 4.0 * heft) * rise * rise
    })
    .collect()
}

fn hit(seed: u64) -> Vec<f32> {
  let mut rng = Rng::new(seed);
  let mut ring: Vec<(Osc, f32, f32, f32)> = (0..5)
    .map(|_| {
      (
        Osc(rng.unit()),
        rng.range(1500.0, 5200.0),
        rng.range(0.08, 0.25),
        rng.range(0.3, 1.0)
      )
    })
    .collect();
  let (mut thud, mut crack, mut flesh) =
    (Osc::default(), Svf::new(rng.range(2200.0, 3200.0), 1.2), Lag::new(700.0));
  let mut smack = Svf::new(rng.range(500.0, 800.0), 1.0);
  (0..len(0.6))
    .map(|index| {
      let t = time(index);
      let x = rng.signed();
      let body = thud.sine(45.0 + 75.0 * (-t / 0.03).exp()) * perc(t, 0.002, 0.09);
      let meat = flesh.step(x) * 3.0 * perc(t, 0.001, 0.05);
      let snap = crack.step(x).band * perc(t, 0.0005, 0.012);
      let metal = ring
        .iter_mut()
        .map(|(osc, hz, decay, gain)| osc.sine(*hz) * perc(t, 0.001, *decay) * *gain)
        .sum::<f32>();
      drive(
        body * 0.5
          + meat * 1.5
          + smack.step(x).band * perc(t, 0.001, 0.03) * 1.5
          + snap * 0.8
          + metal * 0.08,
        2.0
      )
    })
    .collect()
}

fn clang(seed: u64) -> Vec<f32> {
  const PARTIALS: [(f32, f32, f32); 8] = [
    (1.0, 1.0, 1.1),
    (2.32, 0.55, 0.9),
    (2.76, 0.8, 0.8),
    (4.18, 0.5, 0.6),
    (5.4, 0.45, 0.5),
    (6.94, 0.3, 0.35),
    (8.93, 0.25, 0.25),
    (11.2, 0.15, 0.18)
  ];
  let mut rng = Rng::new(seed);
  let base = rng.range(380.0, 470.0);
  let mut bars: Vec<(Osc, Osc, f32, f32, f32)> = PARTIALS
    .iter()
    .map(|&(ratio, gain, decay)| {
      (Osc(rng.unit()), Osc(rng.unit()), base * ratio, gain, decay)
    })
    .collect();
  let (mut click, mut thud) = (Svf::new(3500.0, 0.7), Osc::default());
  (0..len(1.5))
    .map(|index| {
      let t = time(index);
      let ring = bars
        .iter_mut()
        .map(|(a, b, hz, gain, decay)| {
          (a.sine(*hz) + b.sine(*hz * 1.004)) * *gain * perc(t, 0.0008, *decay)
        })
        .sum::<f32>();
      ring * 0.4
        + click.step(rng.signed()).high * perc(t, 0.0003, 0.006)
        + thud.sine(130.0) * perc(t, 0.001, 0.04) * 0.6
    })
    .collect()
}

fn footstep(seed: u64) -> Vec<f32> {
  let mut rng = Rng::new(seed);
  let toe = rng.range(0.035, 0.06);
  let mut soft = Lag::new(rng.range(300.0, 500.0));
  let mut scuff = Svf::new(rng.range(900.0, 1500.0), 0.9);
  let mut grit = Svf::new(rng.range(2500.0, 4000.0), 1.0);
  (0..len(0.22))
    .map(|index| {
      let t = time(index);
      let x = rng.signed();
      let env = perc(t, 0.004, 0.035) + 0.6 * perc(t - toe, 0.004, 0.03);
      let grain = if rng.unit() < 0.1 * env { rng.signed() } else { 0.0 };
      soft.step(x) * 4.0 * env
        + scuff.step(x).band * env * 0.5
        + grit.step(grain).band * 1.5
    })
    .collect()
}

fn clicks(
  into: &mut [f32],
  seed: u64,
  count: usize,
  from: f32,
  to: f32,
  pitch: (f32, f32)
) {
  let mut rng = Rng::new(seed);
  for _ in 0..count {
    let at = len(rng.range(from, to));
    let (hz, mut osc, mut tick) = (
      rng.range(pitch.0, pitch.1),
      Osc::default(),
      Svf::new(rng.range(1800.0, 3500.0), 1.5)
    );
    let click: Vec<f32> = (0..len(0.08))
      .map(|index| {
        let t = time(index);
        osc.sine(hz) * perc(t, 0.0005, 0.018)
          + tick.step(rng.signed()).band * perc(t, 0.0002, 0.004) * 2.0
      })
      .collect();
    mix(into, at, &click, rng.range(0.4, 1.0))
  }
}

fn creak(seed: u64) -> Vec<f32> {
  const WOOD: [(f32, f32); 4] =
    [(260.0, 14.0), (640.0, 12.0), (1400.0, 10.0), (2600.0, 8.0)];
  let mut rng = Rng::new(seed);
  let mut bank = WOOD.map(|(hz, q)| Svf::new(hz, q));
  let mut stick = 0.0f32;
  let mut jitter = Lag::new(8.0);
  let mut latch = Svf::new(4200.0, 2.0);
  let (mut ping, mut knock, mut boards) =
    (Osc::default(), Osc::default(), Lag::new(400.0));
  (0..len(1.4))
    .map(|index| {
      let t = time(index);
      let rate = curve(t, &[(0.0, 70.0), (0.4, 150.0), (0.8, 110.0), (1.05, 55.0)])
        * (1.0 + 0.4 * jitter.step(rng.signed() * 3.0));
      stick += rate / RATE;
      let slip = if stick >= 1.0 { rng.range(0.6, 1.0) } else { 0.0 };
      stick = stick.fract();
      let env =
        curve(t, &[(0.0, 0.0), (0.12, 0.0), (0.18, 1.0), (0.9, 0.8), (1.05, 0.0)]);
      let groan = bank.iter_mut().map(|filter| filter.step(slip * env).band).sum::<f32>();
      let x = rng.signed();
      let click = latch.step(x).band * perc(t, 0.0003, 0.01)
        + ping.sine(2200.0) * perc(t, 0.001, 0.03) * 0.3;
      let thunk =
        (boards.step(x) * 3.0 + knock.sine(110.0)) * perc(t - 1.1, 0.002, 0.06) * 0.3;
      groan * 3.0 + click + thunk
    })
    .collect()
}

fn discover(seed: u64) -> Wave {
  let mut dry = vec![0.0; len(4.5)];
  mix(&mut dry, 0, &swell(&[38.0, 50.0, 57.0, 62.0, 65.0], 2.2, Phone::O, seed), 1.0);
  mix(&mut dry, 0, &drum(55.0, 0.8, seed), 0.2);
  Hall::new(0.85, 0.3, 1.3).apply(&Wave::mono(dry), 0.9, false)
}

fn word_learned(seed: u64) -> Wave {
  let env = Adsr { attack: 1.0, decay: 3.0, sustain: 0.9, release: 1.5 };
  let mut boom = Osc::default();
  let mut rumble = Lag::new(120.0);
  let mut rng = Rng::new(seed);
  let mut dry: Vec<f32> = (0..len(7.0))
    .map(|index| {
      let t = time(index);
      (boom.sine(36.0 + 60.0 * (-t / 0.08).exp()) * perc(t, 0.005, 1.4)
        + rumble.step(rng.signed()) * 6.0 * perc(t, 0.002, 0.35))
        * 0.4
    })
    .collect();
  mix(&mut dry, 0, &drum(45.0, 1.0, seed), 0.25);
  mix(
    &mut dry,
    len(0.1),
    &choir(&[38.0, 45.0, 50.0, 53.0, 57.0, 62.0], 3.5, Phone::A, env, seed),
    0.8
  );
  mix(
    &mut dry,
    len(0.6),
    &choir(&[74.0, 77.0, 81.0], 2.8, Phone::EE, env, seed + 1),
    0.15
  );
  Hall::new(0.92, 0.3, 1.5).apply(&Wave::mono(dry), 1.0, false)
}

fn shout(seed: u64) -> Wave {
  let low = 82.0;
  let fus_ro_dah = [
    Phone::F.voiced(0.0, 0.0).at(0.0),
    Phone::F.at(0.03),
    Phone::F.at(0.11),
    Phone::U.voiced(1.0, 0.25).pitched(low * 1.1).at(0.16),
    Phone::U.voiced(1.0, 0.25).pitched(low * 1.05).at(0.3),
    Phone::S.voiced(0.0, 0.7).pitched(low).at(0.36),
    Phone::S.voiced(0.0, 0.7).at(0.5),
    Phone::S.voiced(0.0, 0.0).at(0.56),
    Phone::R.voiced(0.0, 0.0).pitched(low).at(0.6),
    Phone::R.voiced(0.9, 0.2).pitched(low).at(0.64),
    Phone::R.voiced(0.9, 0.2).pitched(low * 1.05).at(0.72),
    Phone::O.voiced(1.0, 0.25).pitched(low * 1.12).at(0.78),
    Phone::O.voiced(1.0, 0.25).pitched(low).at(0.95),
    Phone::O.voiced(0.0, 0.0).at(1.02),
    Phone::D.voiced(0.15, 0.0).at(1.06),
    Phone::D.voiced(0.15, 0.0).at(1.12),
    Phone::D.voiced(0.2, 1.2).at(1.125),
    Phone::A.voiced(1.2, 0.35).pitched(low * 1.25).at(1.16),
    Phone::A.voiced(1.2, 0.35).pitched(low * 1.15).at(1.5),
    Phone::A.voiced(0.6, 0.5).pitched(low * 0.9).at(1.75),
    Phone::A.voiced(0.0, 0.0).pitched(low * 0.9).at(1.95)
  ];
  let hushed = |phone: Phone| phone.voiced(phone.voice, phone.hiss * 0.6);
  let man = speak(&fus_ro_dah.map(|phone| hushed(phone).sized(0.9)), 0.3, seed);
  let giant = speak(
    &fus_ro_dah.map(|phone| hushed(phone).sized(0.72).pitched(phone.pitch * 0.5)),
    0.55,
    seed + 1
  );
  let blast_at = 1.14;
  let (mut boom, mut air, mut rumble) = (Osc::default(), Svf::default(), Lag::new(90.0));
  let (mut mellow, mut soft) = (Svf::new(2400.0, 0.6), Svf::new(1800.0, 0.6));
  let mut rng = Rng::new(seed);
  let dry: Vec<f32> = (0..len(5.5))
    .map(|index| {
      let t = time(index);
      let voice = man.get(index).copied().unwrap_or(0.0)
        + giant.get(index).copied().unwrap_or(0.0) * 0.9;
      let since = t - blast_at;
      let x = rng.signed();
      air.tune(curve(since, &[(0.0, 120.0), (0.25, 1300.0), (1.4, 220.0)]), 0.9);
      let rush = air.step(x).band
        * curve(since, &[(0.0, 0.0), (0.15, 1.0), (0.6, 0.6), (1.8, 0.0)]);
      let thump = boom.sine(32.0 + 45.0 * (-since.max(0.0) / 0.12).exp())
        * perc(since, 0.015, 0.9)
        * 0.5;
      let roll = rumble.step(x) * 3.5 * perc(since, 0.03, 1.3);
      mellow.step(drive(voice * 1.1, 1.6)).low
        + soft.step(rush * 0.7).low
        + (thump * 1.1 + roll) * (since > 0.0) as u8 as f32
    })
    .collect();
  Hall::new(0.93, 0.45, 1.6).apply(&Wave::mono(dry), 0.8, false)
}

fn roar(seed: u64) -> Wave {
  let beast = |phone: Phone, pitch: f32, voice: f32, hiss: f32, at: f32| {
    phone.sized(0.55).pitched(pitch).voiced(voice, hiss).at(at)
  };
  let screech = |phone: Phone, pitch: f32, voice: f32, hiss: f32, at: f32| {
    phone.sized(1.2).pitched(pitch).voiced(voice, hiss).at(at)
  };
  let low = speak(
    &[
      beast(Phone::UH, 38.0, 0.0, 0.0, 0.0),
      beast(Phone::A, 48.0, 1.0, 0.7, 0.35),
      beast(Phone::A, 56.0, 1.0, 0.8, 1.2),
      beast(Phone::O, 44.0, 1.0, 0.8, 2.2),
      beast(Phone::U, 34.0, 0.3, 0.6, 2.9),
      beast(Phone::U, 32.0, 0.0, 0.0, 3.3)
    ],
    1.0,
    seed
  );
  let high = speak(
    &[
      screech(Phone::EH, 260.0, 0.0, 0.0, 0.2),
      screech(Phone::A, 320.0, 0.8, 0.9, 0.5),
      screech(Phone::A, 360.0, 0.8, 1.0, 1.4),
      screech(Phone::O, 250.0, 0.5, 0.8, 2.4),
      screech(Phone::O, 230.0, 0.0, 0.0, 3.0)
    ],
    1.0,
    seed + 1
  );
  let mut rumble = Lag::new(90.0);
  let mut mellow = Svf::new(2600.0, 0.6);
  let mut rng = Rng::new(seed);
  let dry: Vec<f32> = (0..len(5.0))
    .map(|index| {
      let t = time(index);
      let layer = |wave: &Vec<f32>| wave.get(index).copied().unwrap_or(0.0);
      let ground = rumble.step(rng.signed())
        * 10.0
        * curve(t, &[(0.0, 0.0), (0.5, 1.0), (2.8, 0.6), (3.6, 0.0)]);
      mellow.step(drive(layer(&low) * 1.2 + layer(&high) * 0.2 + ground, 1.8)).low
    })
    .collect();
  Hall::new(0.9, 0.4, 1.4).apply(&Wave::mono(dry), 0.7, false)
}

fn fire_breath(seed: u64) -> Wave {
  let mut rng = Rng::new(seed);
  let dur = 2.8;
  let (mut deep, mut body, mut flutter) =
    (Lag::new(140.0), Svf::new(520.0, 0.7), Lag::new(6.0));
  let mut hiss = Svf::new(1400.0, 0.5);
  let mut crackle = Svf::new(2200.0, 1.2);
  let mut spark = 0.0f32;
  let dry: Vec<f32> = (0..len(dur + 0.6))
    .map(|index| {
      let t = time(index);
      let x = rng.signed();
      let swell =
        curve(t, &[(0.0, 0.0), (0.18, 1.0), (0.5, 0.85), (2.2, 0.75), (dur, 0.0)]);
      let wobble = 0.75 + 0.5 * flutter.step(rng.signed() * 4.0).clamp(-0.5, 0.5);
      spark =
        if rng.unit() < 0.0015 * swell { rng.range(0.4, 1.0) } else { spark * 0.93 };
      let roar = deep.step(x) * 5.0 + body.step(x).band * 1.4;
      let air = hiss.step(x).low * 0.35;
      (roar * wobble + air) * swell + crackle.step(spark * rng.signed()).band * 0.5
    })
    .collect();
  Hall::new(0.7, 0.5, 1.1).apply(&Wave::mono(dry), 0.35, false)
}

fn wingbeat(seed: u64) -> Vec<f32> {
  let mut rng = Rng::new(seed);
  let (mut air, mut feathers, mut thump) =
    (Svf::default(), Svf::new(rng.range(1500.0, 2200.0), 1.0), Osc::default());
  (0..len(0.8))
    .map(|index| {
      let t = time(index);
      let x = rng.signed();
      let env = perc(t - 0.05, 0.07, 0.13);
      air.tune(curve(t, &[(0.0, 1500.0), (0.3, 250.0)]), 0.8);
      air.step(x).low * env * 2.0
        + thump.sine(60.0) * perc(t - 0.07, 0.03, 0.1) * 0.4
        + feathers.step(x).band * env * (0.5 + 0.5 * (t * 220.0).sin()) * 0.3
    })
    .collect()
}

fn level_up(seed: u64) -> Wave {
  let bell = |note: f32| {
    let hz = midi(note);
    let mut partials =
      [(1.0, 1.0, 1.6), (2.0, 0.4, 0.9), (2.76, 0.25, 0.6), (3.0, 0.15, 0.5)]
        .map(|(ratio, gain, decay)| (Osc::default(), hz * ratio, gain, decay));
    (0..len(2.5))
      .map(|index| {
        let t = time(index);
        partials
          .iter_mut()
          .map(|(osc, freq, gain, decay)| {
            osc.sine(*freq) * *gain * perc(t, 0.002, *decay)
          })
          .sum::<f32>()
      })
      .collect::<Vec<f32>>()
  };
  let mut dry = vec![0.0; len(4.0)];
  for (step, &note) in [57.0, 62.0, 66.0, 69.0, 74.0].iter().enumerate() {
    mix(&mut dry, len(step as f32 * 0.11), &bell(note), 0.35)
  }
  mix(&mut dry, 0, &swell(&[50.0, 57.0, 62.0, 66.0], 1.6, Phone::A, seed), 0.8);
  for &note in [62.0, 69.0].iter() {
    mix(&mut dry, len(0.4), &Saws::HORN.play(note, 1.2, seed), 0.35)
  }
  Hall::new(0.85, 0.3, 1.3).apply(&Wave::mono(dry), 0.8, false)
}

fn coins(seed: u64) -> Vec<f32> {
  let mut rng = Rng::new(seed);
  let mut dry = vec![0.0; len(0.8)];
  let count = 5 + rng.below(5);
  for clink in 0..count {
    let at = if clink == 0 { 0.0 } else { rng.range(0.02, 0.38) };
    let mut partials: Vec<(Osc, f32, f32)> = (0..3)
      .map(|_| (Osc(rng.unit()), rng.range(2800.0, 7500.0), rng.range(0.03, 0.12)))
      .collect();
    let mut tick = Svf::new(6000.0, 0.8);
    let sound: Vec<f32> = (0..len(0.4))
      .map(|index| {
        let t = time(index);
        partials
          .iter_mut()
          .map(|(osc, hz, decay)| osc.sine(*hz) * perc(t, 0.0005, *decay))
          .sum::<f32>()
          + tick.step(rng.signed()).high * perc(t, 0.0002, 0.002)
      })
      .collect();
    let gain = rng.range(0.4, 1.0);
    mix(&mut dry, len(at), &sound, gain)
  }
  dry
}

fn wolf_growl(seed: u64) -> Vec<f32> {
  let dog = |phone: Phone, pitch: f32, voice: f32, hiss: f32, at: f32| {
    phone.sized(1.25).pitched(pitch).voiced(voice, hiss).trilled(0.4).at(at)
  };
  let wobble = Rng::new(seed).range(0.9, 1.1);
  speak(
    &[
      dog(Phone::UH, 80.0 * wobble, 0.0, 0.0, 0.0),
      dog(Phone::UH, 88.0 * wobble, 1.0, 0.5, 0.15),
      dog(Phone::O, 95.0 * wobble, 1.0, 0.6, 0.6),
      dog(Phone::UH, 84.0 * wobble, 1.0, 0.5, 1.0),
      dog(Phone::U, 75.0 * wobble, 0.0, 0.1, 1.3)
    ],
    1.0,
    seed
  )
  .into_iter()
  .map(|x| drive(x * 2.0, 3.0))
  .collect()
}

fn wolf_bite(seed: u64) -> Vec<f32> {
  let dog = |phone: Phone, pitch: f32, voice: f32, hiss: f32, at: f32| {
    phone.sized(1.25).pitched(pitch).voiced(voice, hiss).at(at)
  };
  let mut dry = speak(
    &[
      dog(Phone::A, 170.0, 0.0, 0.0, 0.0),
      dog(Phone::A, 200.0, 1.0, 0.6, 0.03),
      dog(Phone::EH, 160.0, 0.8, 0.7, 0.2),
      dog(Phone::EH, 150.0, 0.0, 0.0, 0.32)
    ],
    0.9,
    seed
  );
  dry.resize(len(0.45), 0.0);
  let mut teeth = Svf::new(3500.0, 2.0);
  let mut rng = Rng::new(seed);
  let snap: Vec<f32> = (0..len(0.1))
    .map(|index| {
      let t = time(index);
      teeth.step(rng.signed()).band
        * (perc(t, 0.0003, 0.006) + perc(t - 0.07, 0.0003, 0.008))
        * 3.0
    })
    .collect();
  mix(&mut dry, 0, &snap, 1.0);
  dry
}

fn wolf_die(seed: u64) -> Vec<f32> {
  let pup = |phone: Phone, pitch: f32, voice: f32, hiss: f32, at: f32| {
    phone.sized(1.5).pitched(pitch).voiced(voice, hiss).at(at)
  };
  speak(
    &[
      pup(Phone::EE, 900.0, 0.0, 0.0, 0.0),
      pup(Phone::EH, 1000.0, 1.0, 0.1, 0.02),
      pup(Phone::UH, 650.0, 0.9, 0.15, 0.2),
      pup(Phone::U, 600.0, 0.0, 0.05, 0.3),
      pup(Phone::EE, 750.0, 0.0, 0.0, 0.45),
      pup(Phone::EE, 760.0, 0.7, 0.1, 0.5),
      pup(Phone::U, 480.0, 0.5, 0.1, 0.85),
      pup(Phone::U, 460.0, 0.0, 0.0, 0.95),
      pup(Phone::EE, 600.0, 0.0, 0.0, 1.1),
      pup(Phone::EE, 600.0, 0.4, 0.15, 1.15),
      pup(Phone::U, 380.0, 0.15, 0.2, 1.6),
      pup(Phone::U, 360.0, 0.0, 0.0, 1.8)
    ],
    0.25,
    seed
  )
}

fn draugr(phones: &[(Phone, f32, f32, f32, f32)], seed: u64) -> Vec<f32> {
  let track: Vec<Phone> = phones
    .iter()
    .map(|&(phone, pitch, voice, hiss, at)| {
      phone.sized(0.88).pitched(pitch).voiced(voice, hiss).at(at)
    })
    .collect();
  speak(&track, 1.0, seed).into_iter().map(|x| drive(x * 1.5, 2.5)).collect()
}

fn human(phones: &[(Phone, f32, f32, f32, f32)], rough: f32, seed: u64) -> Vec<f32> {
  let pitch = Rng::new(seed).range(0.92, 1.08);
  let track: Vec<Phone> = phones
    .iter()
    .map(|&(phone, hz, voice, hiss, at)| {
      phone.pitched(hz * pitch).voiced(voice, hiss).at(at)
    })
    .collect();
  speak(&track, rough, seed).into_iter().map(|x| drive(x * 1.5, 2.0)).collect()
}

fn draugr_die(seed: u64) -> Vec<f32> {
  let mut dry = draugr(
    &[
      (Phone::A, 70.0, 0.0, 0.0, 0.0),
      (Phone::A, 72.0, 1.0, 0.4, 0.08),
      (Phone::O, 50.0, 0.8, 0.6, 0.6),
      (Phone::U, 38.0, 0.2, 0.7, 1.1),
      (Phone::U, 36.0, 0.0, 0.0, 1.5)
    ],
    seed
  );
  dry.resize(len(2.0), 0.0);
  clicks(&mut dry, seed, 8, 1.0, 1.75, (700.0, 1400.0));
  dry
}

fn gunshot(seed: u64) -> Vec<f32> {
  let mut rng = Rng::new(seed);
  let (mut crack, mut body, mut thump) =
    (Svf::new(rng.range(1800.0, 2600.0), 0.9), Lag::new(900.0), Osc::default());
  (0..len(0.3))
    .map(|index| {
      let t = time(index);
      let x = rng.signed();
      drive(
        crack.step(x).band * perc(t, 0.0003, 0.012) * 2.0
          + body.step(x) * 4.0 * perc(t, 0.001, 0.07)
          + thump.sine(60.0 + 140.0 * (-t / 0.015).exp()) * perc(t, 0.001, 0.05),
        3.0
      )
    })
    .collect()
}

fn explosion(seed: u64) -> Vec<f32> {
  let mut rng = Rng::new(seed);
  let (mut rumble, mut roar, mut debris, mut boom) =
    (Lag::new(120.0), Lag::new(500.0), Svf::new(3000.0, 1.5), Osc::default());
  (0..len(3.5))
    .map(|index| {
      let t = time(index);
      let x = rng.signed();
      let crackle = (rng.unit() < 0.004 * (-t / 1.2).exp()) as u8 as f32 * rng.signed();
      drive(
        rumble.step(x) * 14.0 * perc(t, 0.004, 1.1)
          + roar.step(x) * 5.0 * perc(t, 0.002, 0.35)
          + boom.sine(30.0 + 70.0 * (-t / 0.12).exp()) * perc(t, 0.002, 0.6) * 1.2
          + debris.step(crackle).band * 6.0
          + x * perc(t, 0.0005, 0.02),
        2.5
      )
    })
    .collect()
}

pub fn variants(cue: Cue) -> u64 {
  match cue {
    Cue::Footstep => 6,
    Cue::Swing | Cue::Hit | Cue::PlayerHurt | Cue::Coins => 3,
    Cue::Block
    | Cue::WolfGrowl
    | Cue::DraugrGroan
    | Cue::BanditShout
    | Cue::Wingbeat
    | Cue::PowerSwing => 2,
    Cue::Gunshot => 4,
    _ => 1
  }
}

pub fn render(cue: Cue, seed: u64) -> Wave {
  let hurt = [
    (Phone::UH, 115.0, 0.0, 0.3, 0.0),
    (Phone::UH, 135.0, 1.0, 0.2, 0.04),
    (Phone::UH, 125.0, 0.9, 0.2, 0.16),
    (Phone::U, 95.0, 0.0, 0.1, 0.32)
  ];
  let hah = [
    (Phone::A.sized(1.08), 150.0, 0.0, 0.0, 0.0),
    (Phone::A.sized(1.08), 150.0, 0.0, 0.6, 0.05),
    (Phone::A.sized(1.08), 185.0, 1.0, 0.25, 0.1),
    (Phone::A.sized(1.08), 175.0, 1.0, 0.2, 0.26),
    (Phone::A.sized(1.08), 120.0, 0.6, 0.3, 0.36),
    (Phone::A.sized(1.08), 110.0, 0.0, 0.0, 0.45)
  ];
  let dying = [
    (Phone::UH, 140.0, 0.0, 0.0, 0.0),
    (Phone::UH, 160.0, 1.0, 0.2, 0.06),
    (Phone::A, 150.0, 1.0, 0.25, 0.35),
    (Phone::O, 105.0, 0.7, 0.35, 0.75),
    (Phone::U, 80.0, 0.2, 0.3, 1.05),
    (Phone::U, 70.0, 0.0, 0.0, 1.3)
  ];
  let groan = [
    (Phone::O, 52.0, 0.0, 0.0, 0.0),
    (Phone::O, 58.0, 0.9, 0.5, 0.25),
    (Phone::U, 60.0, 1.0, 0.45, 0.9),
    (Phone::O, 50.0, 0.8, 0.5, 1.3),
    (Phone::O, 45.0, 0.0, 0.0, 1.7)
  ];
  let inhale = [
    (Phone::A, 45.0, 0.0, 0.0, 0.0),
    (Phone::EH, 45.0, 0.08, 0.4, 0.4),
    (Phone::A, 50.0, 0.15, 0.9, 1.0),
    (Phone::O, 60.0, 0.9, 0.5, 1.25),
    (Phone::O, 55.0, 0.0, 0.0, 1.6)
  ];
  let wave = match cue {
    Cue::Swing => Wave::mono(whoosh(seed, 0.3, 1.0, 0.1)),
    Cue::PowerSwing => Wave::mono(whoosh(seed, 0.5, 0.6, 0.6)),
    Cue::Hit => Wave::mono(hit(seed)),
    Cue::Block => Wave::mono(clang(seed)),
    Cue::Footstep => Wave::mono(footstep(seed)),
    Cue::WolfGrowl => Wave::mono(wolf_growl(seed)),
    Cue::WolfBite => Wave::mono(wolf_bite(seed)),
    Cue::WolfDie => Wave::mono(wolf_die(seed)),
    Cue::DraugrWake => Wave::mono(draugr(&inhale, seed)),
    Cue::DraugrGroan => Wave::mono(draugr(&groan, seed)),
    Cue::DraugrDie => Wave::mono(draugr_die(seed)),
    Cue::BanditShout => Wave::mono(human(&hah, 0.35, seed)),
    Cue::ManDie => Wave::mono(human(&dying, 0.55, seed)),
    Cue::PlayerHurt => Wave::mono(human(&hurt, 0.4, seed)),
    Cue::ChestOpen => Wave::mono(creak(seed)),
    Cue::Discover => discover(seed),
    Cue::WordWall => chant(7.0, false, seed),
    Cue::WordLearned => word_learned(seed),
    Cue::Shout => shout(seed),
    Cue::DragonRoar => roar(seed),
    Cue::FireBreath => fire_breath(seed),
    Cue::Wingbeat => Wave::mono(wingbeat(seed)),
    Cue::LevelUp => level_up(seed),
    Cue::Coins => Wave::mono(coins(seed)),
    Cue::Gunshot => Wave::mono(gunshot(seed)),
    Cue::Explosion => Wave::mono(explosion(seed))
  };
  wave.trimmed().normalized(0.9)
}
