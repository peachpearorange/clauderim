use {crate::{model::{self, Piece, ball, cone, sculpt},
             noise::{self, Roll},
             stuff::Stuff},
     bevy::prelude::*};

#[derive(Clone, Copy)]
struct Coat {
  hide: Srgba,
  hair: Srgba,
  socks: Srgba
}

const COATS: [Coat; 5] = [
  Coat {
    hide: Srgba::new(0.36, 0.2, 0.11, 1.0),
    hair: Srgba::new(0.06, 0.05, 0.05, 1.0),
    socks: Srgba::new(0.08, 0.06, 0.05, 1.0)
  },
  Coat {
    hide: Srgba::new(0.5, 0.28, 0.13, 1.0),
    hair: Srgba::new(0.55, 0.36, 0.2, 1.0),
    socks: Srgba::new(0.5, 0.28, 0.13, 1.0)
  },
  Coat {
    hide: Srgba::new(0.08, 0.07, 0.07, 1.0),
    hair: Srgba::new(0.05, 0.05, 0.05, 1.0),
    socks: Srgba::new(0.08, 0.07, 0.07, 1.0)
  },
  Coat {
    hide: Srgba::new(0.62, 0.6, 0.57, 1.0),
    hair: Srgba::new(0.78, 0.77, 0.74, 1.0),
    socks: Srgba::new(0.34, 0.33, 0.32, 1.0)
  },
  Coat {
    hide: Srgba::new(0.58, 0.47, 0.32, 1.0),
    hair: Srgba::new(0.12, 0.1, 0.08, 1.0),
    socks: Srgba::new(0.16, 0.13, 0.1, 1.0)
  }
];

fn key(x: f32, y: f32, z: f32, wide: f32, high: f32, deep: f32) -> (Vec3, Vec3) {
  (Vec3::new(x, y, z), Vec3::new(wide, high, deep))
}

fn groomed(mesh: Mesh, coat: Coat, dapple: f32, seed: u32) -> Piece {
  let (hide, socks) = (LinearRgba::from(coat.hide), LinearRgba::from(coat.socks));
  Piece::new(model::ruffled(mesh, 0.006, Vec3::new(14.0, 9.0, 14.0), seed), coat.hide)
    .grained(2.0)
    .shaded(move |position, normal| {
      let under = ((-normal.y + 0.2) * 1.2).clamp(0.0, 1.0) * 0.2;
      let sock = ((0.42 - position.y) * 5.0).clamp(0.0, 1.0);
      let spots = (noise::value3(position * 7.0, seed) - 0.5) * dapple;
      let sheen = noise::value3(position * Vec3::new(2.0, 8.0, 2.0), seed + 3) * 0.12;
      (hide * (1.0 - under + spots + sheen)).mix(&socks, sock).with_alpha(1.0)
    })
}

pub fn horse(roll: &mut Roll, saddled: bool) -> Vec<(Stuff, Piece)> {
  let coat = COATS[roll.below(COATS.len())];
  let dapple = roll.chance(0.3) as u8 as f32 * 0.35;
  let seed = roll.below(1000) as u32;
  let graze = roll.range(0.0, 0.5);
  let body = sculpt(
    &[
      key(0.0, 1.3, -0.97, 0.004, 0.004, 0.004),
      key(0.0, 1.3, -0.9, 0.17, 0.17, 0.2),
      key(0.0, 1.28, -0.72, 0.27, 0.22, 0.34),
      key(0.0, 1.22, -0.42, 0.3, 0.2, 0.37),
      key(0.0, 1.2, -0.05, 0.31, 0.18, 0.4),
      key(0.0, 1.24, 0.3, 0.29, 0.2, 0.4),
      key(0.0, 1.3, 0.56, 0.24, 0.22, 0.38),
      key(0.0, 1.33, 0.72, 0.16, 0.18, 0.26),
      key(0.0, 1.34, 0.8, 0.004, 0.004, 0.004)
    ],
    6,
    18
  );
  let lift = 1.0 - graze;
  let poll = Vec3::new(0.0, 1.35 + 0.55 * lift, 0.95 + 0.3 * graze);
  let neck = sculpt(
    &[
      key(0.0, 1.25, 0.42, 0.004, 0.004, 0.004),
      key(0.0, 1.35, 0.52, 0.15, 0.2, 0.26),
      (Vec3::new(0.0, 1.35, 0.55).lerp(poll, 0.4), Vec3::new(0.12, 0.13, 0.19)),
      (Vec3::new(0.0, 1.35, 0.55).lerp(poll, 0.78), Vec3::new(0.095, 0.1, 0.13)),
      (poll, Vec3::splat(0.07)),
      (poll + Vec3::new(0.0, 0.03, 0.03), Vec3::splat(0.004))
    ],
    5,
    14
  );
  let muzzle = poll + Vec3::new(0.0, -0.42 - 0.2 * graze, 0.36 - 0.12 * graze);
  let head = sculpt(
    &[
      (poll + Vec3::new(0.0, 0.05, -0.04), Vec3::splat(0.004)),
      (poll, Vec3::new(0.09, 0.08, 0.1)),
      (poll.lerp(muzzle, 0.35), Vec3::new(0.085, 0.07, 0.09)),
      (poll.lerp(muzzle, 0.75), Vec3::new(0.06, 0.055, 0.06)),
      (muzzle, Vec3::new(0.06, 0.055, 0.06)),
      (muzzle + (muzzle - poll).normalize() * 0.05, Vec3::splat(0.004))
    ],
    5,
    14
  );
  let leg = |x: f32, z: f32, hind: bool| {
    let bend = hind.then_some(-0.12).unwrap_or(0.02);
    sculpt(
      &[
        key(x, 1.18, z, 0.004, 0.004, 0.004),
        key(x, 1.05, z, 0.09, 0.14, 0.12),
        key(x, 0.82, z + bend * 0.6, 0.07, 0.1, 0.09),
        key(x, 0.56, z + bend, 0.045, 0.05, 0.055),
        key(x, 0.32, z + bend * 0.4, 0.034, 0.038, 0.036),
        key(x, 0.15, z + 0.02, 0.043, 0.048, 0.045),
        key(x, 0.07, z + 0.035, 0.052, 0.055, 0.05),
        key(x, 0.0, z + 0.04, 0.004, 0.004, 0.004)
      ],
      4,
      10
    )
  };
  let legs =
    [(0.16, 0.48, false), (-0.16, 0.48, false), (0.16, -0.7, true), (-0.16, -0.7, true)]
      .map(|(x, z, hind)| leg(x, z, hind));
  let hooves =
    [(0.16, 0.52), (-0.16, 0.52), (0.16, -0.66), (-0.16, -0.66)].map(|(x, z)| {
      Piece::new(model::rod(0.058, 0.08), Srgba::new(0.1, 0.09, 0.08, 1.0))
        .at_xyz(x, 0.04, z)
    });
  let mane_path: Vec<(Vec3, Vec3)> = (0..=6)
    .map(|step| {
      let t = step as f32 / 6.0;
      let at = Vec3::new(0.0, 1.47, 0.58).lerp(poll + Vec3::new(0.0, 0.07, -0.02), t);
      let size = (t * (1.0 - t) * 4.0).max(0.05);
      (at, Vec3::new(0.03 * size + 0.004, 0.07 * size + 0.004, 0.02 * size + 0.004))
    })
    .collect();
  let mane =
    model::ruffled(sculpt(&mane_path, 3, 8), 0.02, Vec3::new(30.0, 6.0, 30.0), seed);
  let sway = roll.spread(0.06);
  let tail = model::ruffled(
    sculpt(
      &[
        key(0.0, 1.32, -0.9, 0.004, 0.004, 0.004),
        key(0.0, 1.3, -0.98, 0.04, 0.04, 0.04),
        key(sway * 0.5, 1.1, -1.06, 0.07, 0.08, 0.06),
        key(sway, 0.8, -1.08, 0.09, 0.1, 0.07),
        key(sway * 1.2, 0.58, -1.05, 0.06, 0.06, 0.05),
        key(sway * 1.2, 0.5, -1.04, 0.004, 0.004, 0.004)
      ],
      4,
      10
    ),
    0.03,
    Vec3::new(40.0, 5.0, 40.0),
    seed + 1
  );
  let ears = [1.0, -1.0].map(|side| {
    Piece::new(cone(0.035, 0.13), coat.hide)
      .sized(Vec3::new(1.0, 1.0, 0.5))
      .rolled(-side * 0.25)
      .at(poll + Vec3::new(side * 0.05, 0.08, -0.02))
  });
  let across = (muzzle - poll).normalize().cross(Vec3::X).normalize();
  let eyes = [1.0, -1.0].map(|side| {
    Piece::new(ball(0.022), Srgba::new(0.03, 0.02, 0.02, 1.0))
      .at(poll.lerp(muzzle, 0.22) + Vec3::X * side * 0.085 - across * 0.01)
  });
  let nostrils = [1.0, -1.0].map(|side| {
    Piece::new(ball(0.016), Srgba::new(0.04, 0.03, 0.03, 1.0))
      .at(muzzle + Vec3::X * side * 0.035 + (muzzle - poll).normalize() * 0.03)
  });
  let hide = [body, neck, head]
    .into_iter()
    .chain(legs)
    .enumerate()
    .map(|(index, mesh)| {
      (Stuff::Fur, groomed(mesh, coat, dapple, seed + index as u32 * 7))
    })
    .chain(ears.map(|ear| (Stuff::Fur, ear)))
    .chain(
      [mane, tail].map(|mesh| (Stuff::Fur, Piece::new(mesh, coat.hair).grained(4.0)))
    )
    .chain(hooves.map(|hoof| (Stuff::Bone, hoof)))
    .chain(eyes.into_iter().chain(nostrils).map(|dot| (Stuff::Gloss, dot)));
  let tack = saddled
    .then(|| {
      let leather = Srgba::new(0.22, 0.13, 0.08, 1.0);
      let blanket = [
        Srgba::new(0.42, 0.12, 0.1, 1.0),
        Srgba::new(0.16, 0.22, 0.34, 1.0),
        Srgba::new(0.3, 0.3, 0.22, 1.0)
      ][roll.below(3)];
      vec![
        (
          Stuff::Cloth,
          Piece::new(model::block(0.7, 0.03, 0.62), blanket)
            .sized(Vec3::ONE)
            .at_xyz(0.0, 1.5, 0.08)
        ),
        (
          Stuff::Leather,
          Piece::new(ball(1.0), leather)
            .sized(Vec3::new(0.3, 0.08, 0.3))
            .at_xyz(0.0, 1.53, 0.08)
        ),
        (
          Stuff::Leather,
          Piece::new(ball(1.0), leather)
            .sized(Vec3::new(0.12, 0.08, 0.06))
            .at_xyz(0.0, 1.6, 0.28)
        ),
        (
          Stuff::Leather,
          Piece::new(model::block(0.66, 0.05, 0.06), leather).at_xyz(0.0, 1.22, 0.1)
        ),
      ]
    })
    .unwrap_or_default();
  hide.chain(tack).collect()
}
