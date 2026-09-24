use {crate::{combat::{Dead, Side, Struck, Vitals},
             creature::Foe,
             opts::opts,
             place::{Marker, Place},
             player::{Player, View},
             signal::{Cue, Discovered, Engaged, Notice, Prompt, Sound}},
     bevy::{prelude::*,
            text::{FontSize, FontSource, FontStyle, LetterSpacing},
            ui::{UiSystems,
                 Val::{Percent, Px, VMin, Vw},
                 Val2},
            window::PrimaryWindow},
     std::{collections::{HashMap, VecDeque},
           f32::consts::{FRAC_PI_2, PI, TAU}}};

const PLACE_RANGE: f32 = 900.0;
const HOSTILE_RANGE: f32 = 40.0;
const DOTS: usize = 6;
const REACH: f32 = 2.2;
const FORGET: f32 = 8.0;
const BANNER_TIME: f32 = 4.2;
const NOTICE_TIME: f32 = 4.2;
const INTRO_TIME: f32 = 6.4;

const INK: Color = Color::srgba(0.95, 0.94, 0.91, 0.96);
const PALE: Color = Color::srgba(0.84, 0.83, 0.8, 0.82);
const DIM: Color = Color::srgba(0.8, 0.79, 0.76, 0.5);
const FRAME: Color = Color::srgba(0.74, 0.73, 0.7, 0.55);
const TRACK: Color = Color::srgba(0.0, 0.0, 0.0, 0.45);
const GLOOM: Color = Color::srgba(0.0, 0.0, 0.0, 0.58);
const HOSTILE: Color = Color::srgb(0.86, 0.12, 0.08);

const HINTS: [&str; 6] = [
  "Draugr are the restless dead of the old Nord barrows. They sleep in their alcoves for \
   centuries, and wake to the footfall of the living.",
  "Wolves rarely hunt alone. Where one bares its teeth on the road, the rest of the pack is \
   already circling beyond the pines.",
  "Word Walls hold the language of dragons, carved by priests who served them. A Dragonborn \
   who reads the claw marks can take a Word of Power as their own.",
  "The Standing Stones are older than any hold. The Warrior Stone favours those who would \
   rather settle matters with steel.",
  "Raise your shield against a heavy blow. A well-timed block turns aside even the fury of a \
   draugr overlord.",
  "Bandits make their camps within sight of the roads, and a lone traveller is worth more to \
   them than a dozen honest days of work."
];

#[derive(Resource, Default)]
pub struct Charted(pub Vec<Place>);

#[derive(Resource, Default)]
struct Contact(HashMap<Entity, f32>);

#[derive(Resource, Default)]
struct Rival {
  foe: Option<Entity>,
  fill: f32,
  linger: f32
}

#[derive(Resource, Clone)]
struct Fonts {
  sans: Handle<Font>,
  light: Handle<Font>,
  serif: Handle<Font>,
  serif_light: Handle<Font>
}

#[derive(Component)]
struct Hud;

#[derive(Component)]
struct Fade(f32);

#[derive(Component)]
struct Paint {
  text: Option<Color>,
  back: Option<Color>,
  border: Option<BorderColor>,
  shadow: Option<BoxShadow>,
  text_shadow: Option<TextShadow>,
  gradient: Option<BackgroundGradient>
}

#[derive(Component)]
#[require(LetterSpacing)]
struct Tracking(f32);

#[derive(Component, Clone, Copy)]
enum Heading {
  Fixed(f32),
  Toward(Place),
  Hostile(usize)
}

#[derive(Clone, Copy, PartialEq)]
enum Stroke {
  Solid,
  Line
}

#[derive(Component)]
struct Ink {
  place: Place,
  stroke: Stroke
}

#[derive(Clone, Copy)]
enum Stat {
  Health,
  Magicka,
  Stamina
}

impl Stat {
  fn of(self, vitals: &Vitals) -> f32 {
    let &Vitals { health, health_max, stamina, stamina_max, magicka, magicka_max } =
      vitals;
    match self {
      Stat::Health => health / health_max,
      Stat::Magicka => magicka / magicka_max,
      Stat::Stamina => stamina / stamina_max
    }
    .clamp(0.0, 1.0)
  }

  fn hues(self) -> [Color; 2] {
    match self {
      Stat::Health => [Color::srgb(0.8, 0.2, 0.15), Color::srgb(0.42, 0.06, 0.05)],
      Stat::Magicka => [Color::srgb(0.32, 0.5, 0.9), Color::srgb(0.1, 0.18, 0.5)],
      Stat::Stamina => [Color::srgb(0.4, 0.74, 0.32), Color::srgb(0.13, 0.36, 0.1)]
    }
  }
}

#[derive(Clone, Copy)]
enum Gauge {
  Own(Stat),
  Rival
}

#[derive(Component)]
struct Fill {
  gauge: Gauge,
  shown: f32
}

#[derive(Component)]
struct Meter {
  stat: Stat,
  last: f32,
  idle: f32
}

#[derive(Component)]
struct RivalBar;

#[derive(Component)]
struct RivalName;

#[derive(Component)]
struct Banner;

#[derive(Component)]
struct BannerName;

#[derive(Component)]
struct Board;

#[derive(Component)]
struct Fleeting(f32);

#[derive(Component)]
struct PromptBox;

#[derive(Component)]
struct PromptVerb;

#[derive(Component)]
struct PromptNoun;

#[derive(Component)]
struct Crosshair;

#[derive(Component)]
struct Mourning;

#[derive(Component)]
struct Intro;

#[derive(Component)]
struct IntroTitle;

fn smoothstep(from: f32, to: f32, t: f32) -> f32 {
  let x = ((t - from) / (to - from)).clamp(0.0, 1.0);
  x * x * (3.0 - 2.0 * x)
}

fn envelope(t: f32, [rise, full, fall, gone]: [f32; 4]) -> f32 {
  smoothstep(rise, full, t) * (1.0 - smoothstep(fall, gone, t))
}

fn approach(from: f32, to: f32, step: f32) -> f32 {
  from + (to - from).clamp(-step, step)
}

fn bearing(toward: Vec2) -> f32 { toward.x.atan2(-toward.y) }

fn at(left: f32, top: f32) -> Node {
  Node {
    position_type: PositionType::Absolute,
    left: Percent(left),
    top: Percent(top),
    ..default()
  }
}

fn centred() -> UiTransform { UiTransform::from_translation(Val2::percent(-50.0, -50.0)) }

fn words(
  font: &Handle<Font>,
  size: f32,
  color: Color,
  text: impl Into<String>
) -> impl Bundle {
  (
    Text::new(text),
    TextFont {
      font: FontSource::Handle(font.clone()),
      font_size: FontSize::VMin(size),
      ..default()
    },
    TextColor(color),
    TextShadow { offset: Vec2::new(0.0, 1.5), color: Color::srgba(0.0, 0.0, 0.0, 0.7) }
  )
}

fn spaced(vmin: f32) -> impl Bundle {
  (Tracking(vmin), Node { padding: UiRect::left(VMin(vmin)), ..default() })
}

fn diamond(size: f32, place: Node) -> impl Bundle {
  (
    Node { width: VMin(size), height: VMin(size), border: UiRect::all(Px(1.0)), ..place },
    BackgroundColor(Color::srgba(0.06, 0.06, 0.06, 0.85)),
    BorderColor::all(FRAME),
    UiTransform { rotation: Rot2::degrees(45.0), ..centred() }
  )
}

fn fading_line(horizontal: bool, color: Color) -> BackgroundGradient {
  let stops = vec![
    ColorStop::new(Color::NONE, Percent(0.0)),
    ColorStop::new(color, Percent(18.0)),
    ColorStop::new(color, Percent(82.0)),
    ColorStop::new(Color::NONE, Percent(100.0)),
  ];
  BackgroundGradient::from(
    horizontal
      .then(|| LinearGradient::to_right(stops.clone()))
      .unwrap_or(LinearGradient::to_bottom(stops))
  )
}

fn gauge(
  parent: &mut ChildSpawnerCommands,
  width: f32,
  gauge: Gauge,
  [top, bottom]: [Color; 2]
) {
  parent
    .spawn((
      Node {
        width: VMin(width),
        height: VMin(1.2),
        padding: UiRect::all(Px(2.0)),
        border: UiRect::all(Px(1.0)),
        ..default()
      },
      BorderColor::all(FRAME),
      BackgroundColor(TRACK),
      BoxShadow::new(
        Color::srgba(0.0, 0.0, 0.0, 0.5),
        Px(0.0),
        Px(0.0),
        Px(0.0),
        VMin(0.9)
      )
    ))
    .with_children(|frame| {
      frame
        .spawn(Node { flex_grow: 1.0, height: Percent(100.0), ..default() })
        .with_children(|channel| {
          channel.spawn((
            Node {
              position_type: PositionType::Absolute,
              height: Percent(100.0),
              ..default()
            },
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
              ColorStop::auto(top),
              ColorStop::auto(bottom),
            ])),
            Fill { gauge, shown: 1.0 }
          ));
        });
      frame.spawn(diamond(1.15, at(0.0, 50.0)));
      frame.spawn(diamond(1.15, at(100.0, 50.0)));
    });
}

fn cap(parent: &mut ChildSpawnerCommands) {
  parent.spawn(Node { width: VMin(1.8), height: VMin(4.4), ..default() }).with_children(
    |cap| {
      cap.spawn((
        Node { width: Px(1.0), height: Percent(100.0), ..at(50.0, 0.0) },
        UiTransform::from_translation(Val2::percent(-50.0, 0.0)),
        fading_line(false, FRAME)
      ));
      cap.spawn(diamond(0.85, at(50.0, 50.0)));
    }
  );
}

fn icon(mark: &mut ChildSpawnerCommands, place: Place) {
  let part = |stroke: Stroke, node: Node, transform: UiTransform| {
    (
      Ink { place, stroke },
      node,
      transform,
      BackgroundColor(Color::NONE),
      BorderColor::all(DIM)
    )
  };
  let outline = UiRect::all(Px(1.5));
  mark
    .spawn(Node {
      width: VMin(1.9),
      height: VMin(1.9),
      flex_direction: FlexDirection::Column,
      justify_content: JustifyContent::Center,
      align_items: AlignItems::Center,
      ..default()
    })
    .with_children(|glyph| match place.marker() {
      Marker::Cave => {
        glyph.spawn(part(
          Stroke::Solid,
          Node {
            width: VMin(1.2),
            height: VMin(1.35),
            border: UiRect { bottom: Px(0.0), ..outline },
            border_radius: BorderRadius::top(VMin(0.6)),
            ..default()
          },
          UiTransform::IDENTITY
        ));
      }
      Marker::Barrow => {
        glyph.spawn(part(
          Stroke::Line,
          Node { width: VMin(0.26), height: VMin(0.5), ..default() },
          UiTransform::IDENTITY
        ));
        glyph.spawn(part(
          Stroke::Solid,
          Node {
            width: VMin(1.7),
            height: VMin(0.85),
            border: outline,
            border_radius: BorderRadius::top(VMin(0.85)),
            ..default()
          },
          UiTransform::IDENTITY
        ));
      }
      Marker::Tower => {
        glyph.spawn(part(
          Stroke::Line,
          Node { width: VMin(1.05), height: VMin(0.3), ..default() },
          UiTransform::IDENTITY
        ));
        glyph.spawn(part(
          Stroke::Solid,
          Node { width: VMin(0.66), height: VMin(1.15), border: outline, ..default() },
          UiTransform::IDENTITY
        ));
      }
      Marker::Stone => {
        glyph.spawn(part(
          Stroke::Solid,
          Node {
            width: VMin(0.95),
            height: VMin(0.95),
            border: outline,
            ..at(50.0, 50.0)
          },
          UiTransform { rotation: Rot2::degrees(45.0), ..centred() }
        ));
      }
      Marker::Camp => {
        [(28.0, 30.0), (72.0, -30.0)].into_iter().for_each(|(left, lean)| {
          glyph.spawn(part(
            Stroke::Line,
            Node { width: VMin(0.24), height: VMin(1.5), ..at(left, 47.0) },
            UiTransform { rotation: Rot2::degrees(lean), ..centred() }
          ));
        });
        glyph.spawn(part(
          Stroke::Line,
          Node { width: VMin(1.7), height: VMin(0.22), ..at(50.0, 88.0) },
          centred()
        ));
      }
    });
}

fn heading(heading: Heading, top: f32) -> impl Bundle {
  (
    heading,
    Fade(0.0),
    Node {
      justify_content: JustifyContent::Center,
      align_items: AlignItems::Center,
      ..at(50.0, top)
    },
    centred()
  )
}

fn compass(parent: &mut ChildSpawnerCommands, fonts: &Fonts) {
  parent.spawn(Node { align_items: AlignItems::Center, ..default() }).with_children(
    |row| {
      cap(row);
      row
        .spawn((
          Node {
            width: VMin(64.0),
            max_width: Vw(54.0),
            height: VMin(2.9),
            overflow: Overflow::clip(),
            ..default()
          },
          BackgroundGradient::from(LinearGradient::to_right(vec![
            ColorStop::new(Color::srgba(0.0, 0.0, 0.0, 0.22), Percent(0.0)),
            ColorStop::new(GLOOM, Percent(14.0)),
            ColorStop::new(GLOOM, Percent(86.0)),
            ColorStop::new(Color::srgba(0.0, 0.0, 0.0, 0.22), Percent(100.0)),
          ]))
        ))
        .with_children(|bar| {
          [0.0, 100.0].into_iter().for_each(|top| {
            bar.spawn((
              Node { width: Percent(100.0), height: Px(1.0), ..at(0.0, top) },
              UiTransform::from_translation(Val2::percent(0.0, -top)),
              fading_line(true, FRAME)
            ));
          });
          (0..24).for_each(|step| {
            let degrees = step as f32 * 15.0;
            bar.spawn(heading(Heading::Fixed(degrees.to_radians()), 50.0)).with_children(
              |mark| {
                if let Some(&letter) = ["N", "E", "S", "W"].get(step / 6)
                  && step % 6 == 0
                {
                  let north = step == 0;
                  mark.spawn(words(
                    &fonts.sans,
                    north.then_some(2.15).unwrap_or(1.7),
                    north.then_some(INK).unwrap_or(PALE),
                    letter
                  ))
                } else {
                  mark.spawn((
                    Node {
                      width: Px(1.0),
                      height: VMin((step % 3 == 0).then_some(0.85).unwrap_or(0.5)),
                      ..default()
                    },
                    BackgroundColor(DIM)
                  ))
                };
              }
            );
          });
          Place::ALL.into_iter().for_each(|place| {
            bar
              .spawn(heading(Heading::Toward(place), 50.0))
              .with_children(|mark| icon(mark, place));
          });
          (0..DOTS).for_each(|index| {
            bar.spawn(heading(Heading::Hostile(index), 64.0)).with_children(|mark| {
              mark.spawn((
                Node {
                  width: VMin(0.7),
                  height: VMin(0.7),
                  border_radius: BorderRadius::MAX,
                  ..default()
                },
                BackgroundColor(HOSTILE),
                BoxShadow::new(
                  Color::srgba(0.0, 0.0, 0.0, 0.7),
                  Px(0.0),
                  Px(0.0),
                  Px(0.0),
                  VMin(0.3)
                )
              ));
            });
          });
        });
      cap(row);
    }
  );
}

fn raise(mut commands: Commands, assets: Res<AssetServer>) {
  let fonts = Fonts {
    sans: assets.load("fonts/NotoSans-SemiCondensed.ttf"),
    light: assets.load("fonts/NotoSans-SemiCondensedLight.ttf"),
    serif: assets.load("fonts/NotoSerifDisplay-Regular.ttf"),
    serif_light: assets.load("fonts/NotoSerifDisplay-Light.ttf")
  };
  commands.insert_resource(fonts.clone());
  let whole = Node {
    position_type: PositionType::Absolute,
    width: Percent(100.0),
    height: Percent(100.0),
    ..default()
  };
  let across = |top: f32| Node {
    position_type: PositionType::Absolute,
    top: Percent(top),
    left: Percent(0.0),
    right: Percent(0.0),
    flex_direction: FlexDirection::Column,
    align_items: AlignItems::Center,
    ..default()
  };
  commands.spawn((Name::new("Hud"), Hud, whole.clone())).with_children(|hud| {
    hud.spawn(Node { top: VMin(2.6), row_gap: VMin(1.2), ..across(0.0) }).with_children(
      |top| {
        compass(top, &fonts);
        top
          .spawn((RivalBar, Fade(0.0), Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: VMin(0.5),
            ..default()
          }))
          .with_children(|rival| {
            rival.spawn((RivalName, words(&fonts.sans, 1.75, INK, ""), spaced(0.25)));
            gauge(rival, 24.0, Gauge::Rival, Stat::Health.hues());
          });
      }
    );

    hud.spawn((Banner, Fade(0.0), across(21.0))).with_children(|banner| {
      banner
        .spawn((
          Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            padding: UiRect::axes(VMin(9.0), VMin(2.4)),
            row_gap: VMin(0.4),
            ..default()
          },
          BackgroundGradient::from(RadialGradient {
            stops: vec![
              ColorStop::new(Color::srgba(0.0, 0.0, 0.0, 0.32), Percent(0.0)),
              ColorStop::new(Color::NONE, Percent(70.0)),
            ],
            ..default()
          })
        ))
        .with_children(|plate| {
          plate.spawn((BannerName, words(&fonts.serif, 4.6, INK, ""), spaced(0.7)));
          plate.spawn((words(&fonts.sans, 1.7, PALE, "DISCOVERED"), spaced(0.55)));
        });
    });

    hud.spawn((Board, Node {
      position_type: PositionType::Absolute,
      top: VMin(8.0),
      left: VMin(3.6),
      flex_direction: FlexDirection::Column,
      row_gap: VMin(0.35),
      ..default()
    }));

    hud
      .spawn((PromptBox, Fade(0.0), Node {
        flex_direction: FlexDirection::Row,
        justify_content: JustifyContent::Center,
        column_gap: VMin(1.0),
        ..across(61.0)
      }))
      .with_children(|prompt| {
        prompt
          .spawn((
            Node {
              padding: UiRect::axes(VMin(0.55), VMin(0.05)),
              border: UiRect::all(Px(1.0)),
              border_radius: BorderRadius::all(Px(2.0)),
              ..default()
            },
            BorderColor::all(PALE),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.35))
          ))
          .with_children(|key| {
            key.spawn(words(&fonts.sans, 1.6, INK, "E"));
          });
        prompt.spawn((PromptVerb, words(&fonts.light, 2.0, PALE, "")));
        prompt.spawn((PromptNoun, words(&fonts.sans, 2.0, INK, "")));
      });

    hud.spawn((
      Crosshair,
      Fade(0.0),
      Node {
        width: VMin(0.42),
        height: VMin(0.42),
        border_radius: BorderRadius::MAX,
        ..at(50.0, 50.0)
      },
      centred(),
      BackgroundColor(Color::srgba(0.95, 0.94, 0.9, 0.75)),
      BoxShadow::new(
        Color::srgba(0.0, 0.0, 0.0, 0.6),
        Px(0.0),
        Px(0.0),
        Px(0.0),
        Px(2.0)
      )
    ));

    hud
      .spawn(Node {
        position_type: PositionType::Absolute,
        bottom: VMin(5.5),
        left: Percent(0.0),
        right: Percent(0.0),
        height: VMin(1.2),
        ..default()
      })
      .with_children(|vitals| {
        [
          (Stat::Magicka, Node {
            position_type: PositionType::Absolute,
            left: Vw(4.0),
            ..default()
          }),
          (Stat::Health, Node { ..at(50.0, 0.0) }),
          (Stat::Stamina, Node {
            position_type: PositionType::Absolute,
            right: Vw(4.0),
            ..default()
          })
        ]
        .into_iter()
        .for_each(|(stat, place)| {
          let shift = matches!(stat, Stat::Health).then_some(-50.0).unwrap_or(0.0);
          vitals
            .spawn((
              Meter { stat, last: 1.0, idle: 0.0 },
              Fade(0.0),
              place,
              UiTransform::from_translation(Val2::percent(shift, 0.0))
            ))
            .with_children(|meter| gauge(meter, 28.0, Gauge::Own(stat), stat.hues()));
        });
      });

    hud.spawn((
      Mourning,
      Fade(0.0),
      whole.clone(),
      BackgroundColor(Color::BLACK),
      GlobalZIndex(150)
    ));

    if opts().intro {
      let hint = HINTS[std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos() as usize)
        % HINTS.len()];
      hud
        .spawn((
          Intro,
          Fade(1.0),
          Node {
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..whole.clone()
          },
          BackgroundColor(Color::BLACK),
          GlobalZIndex(200)
        ))
        .with_children(|veil| {
          veil
            .spawn((IntroTitle, Fade(0.0), Node {
              flex_direction: FlexDirection::Column,
              align_items: AlignItems::Center,
              row_gap: VMin(0.6),
              margin: UiRect::bottom(VMin(6.0)),
              ..default()
            }))
            .with_children(|title| {
              title.spawn((
                words(&fonts.serif_light, 1.9, PALE, "THE ELDER SCROLLS"),
                spaced(0.9)
              ));
              title.spawn((words(&fonts.serif, 9.0, INK, "SKYRIM II"), spaced(1.3)));
              title
                .spawn(Node { width: VMin(46.0), height: VMin(1.4), ..default() })
                .with_children(|rule| {
                  rule.spawn((
                    Node { width: Percent(100.0), height: Px(1.0), ..at(0.0, 50.0) },
                    fading_line(true, PALE)
                  ));
                  rule.spawn(diamond(0.9, at(50.0, 50.0)));
                });
            });
          veil
            .spawn((IntroTitle, Fade(0.0), Node {
              bottom: Percent(9.0),
              top: Val::Auto,
              ..across(0.0)
            }))
            .with_children(|lore| {
              lore.spawn((
                Text::new(hint),
                TextColor(PALE),
                TextFont {
                  font: FontSource::Handle(fonts.light.clone()),
                  font_size: FontSize::VMin(1.75),
                  style: FontStyle::Italic,
                  ..default()
                },
                TextLayout::justify(Justify::Center),
                Node { max_width: VMin(68.0), ..default() }
              ));
            });
        });
    }
  });
}

fn engage(
  time: Res<Time>,
  mut struck: MessageReader<Struck>,
  player: Single<Entity, With<Player>>,
  foes: Query<(&Side, Has<Dead>)>,
  mut engaged: ResMut<Engaged>,
  mut contact: ResMut<Contact>
) {
  let now = time.elapsed_secs();
  let hero = *player;
  struck
    .read()
    .filter_map(|hit| {
      (hit.attacker == hero)
        .then_some(hit.target)
        .or((hit.target == hero).then_some(hit.attacker))
    })
    .filter(|&foe| foes.get(foe).is_ok_and(|(&side, _)| side == Side::Wild))
    .for_each(|foe| {
      contact.0.insert(foe, now);
      engaged.0 = Some(foe);
    });
  engaged.0 = engaged.0.filter(|foe| {
    foes.get(*foe).is_ok_and(|(_, dead)| !dead)
      && contact.0.get(foe).is_some_and(|&at| now - at < FORGET)
  });
  contact.0.retain(|&foe, at| now - *at < 3.0 * FORGET && foes.contains(foe));
}

fn rival(
  time: Res<Time>,
  engaged: Res<Engaged>,
  foes: Query<(&Vitals, Option<&Name>, Has<Dead>)>,
  mut rival: ResMut<Rival>,
  mut name: Single<&mut Text, With<RivalName>>,
  mut fade: Single<&mut Fade, With<RivalBar>>
) {
  let delta = time.delta_secs();
  if let Some(foe) = engaged.0
    && let Ok((vitals, title, _)) = foes.get(foe)
  {
    rival.foe = Some(foe);
    rival.fill = Stat::Health.of(vitals);
    rival.linger = 1.6;
    name.set_if_neq(Text(title.map_or("Enemy", Name::as_str).into()));
  } else {
    let fallen =
      rival.foe.is_some_and(|foe| foes.get(foe).ok().is_none_or(|(.., dead)| dead));
    rival.fill = fallen.then_some(0.0).unwrap_or(rival.fill);
    rival.linger = fallen.then_some(rival.linger).unwrap_or(0.0) - delta;
  }
  fade.0 = approach(fade.0, (rival.linger > 0.0) as u8 as f32, delta * 3.0);
}

fn discover(
  player: Single<&Transform, With<Player>>,
  mut charted: ResMut<Charted>,
  mut found: MessageWriter<Discovered>,
  mut sound: MessageWriter<Sound>
) {
  let here = player.translation.xz();
  let fresh: Vec<Place> = Place::ALL
    .into_iter()
    .filter(|place| {
      !charted.0.contains(place) && place.spot().distance(here) < place.flat() * REACH
    })
    .collect();
  fresh.into_iter().for_each(|place| {
    charted.0.push(place);
    found.write(Discovered(place));
    sound.write(Sound::flat(Cue::Discover));
  });
}

fn herald(
  time: Res<Time>,
  mut found: MessageReader<Discovered>,
  mut queue: Local<VecDeque<Place>>,
  mut age: Local<Option<f32>>,
  mut fade: Single<&mut Fade, With<Banner>>,
  mut name: Single<&mut Text, With<BannerName>>
) {
  queue.extend(found.read().map(|&Discovered(place)| place));
  *age = age.map(|age| age + time.delta_secs()).filter(|&age| age < BANNER_TIME).or_else(
    || {
      queue.pop_front().map(|place| {
        name.0 = place.name().to_uppercase();
        0.0
      })
    }
  );
  fade.0 =
    age.map_or(0.0, |age| envelope(age, [0.0, 0.8, BANNER_TIME - 1.1, BANNER_TIME]));
}

fn post(
  time: Res<Time>,
  fonts: Res<Fonts>,
  mut commands: Commands,
  mut notices: MessageReader<Notice>,
  board: Single<Entity, With<Board>>,
  mut lines: Query<(Entity, &mut Fleeting, &mut Fade)>
) {
  notices.read().for_each(|Notice(text)| {
    commands.spawn((
      Fleeting(0.0),
      Fade(0.0),
      words(&fonts.sans, 1.85, INK, text.clone()),
      ChildOf(*board)
    ));
  });
  lines.iter_mut().for_each(|(entity, mut age, mut fade)| {
    age.0 += time.delta_secs();
    fade.0 = envelope(age.0, [0.0, 0.3, NOTICE_TIME - 0.9, NOTICE_TIME]);
    if age.0 > NOTICE_TIME {
      commands.entity(entity).despawn();
    }
  });
}

fn offer(
  time: Res<Time>,
  prompt: Res<Prompt>,
  mut fade: Single<&mut Fade, With<PromptBox>>,
  mut verb: Single<&mut Text, (With<PromptVerb>, Without<PromptNoun>)>,
  mut noun: Single<&mut Text, (With<PromptNoun>, Without<PromptVerb>)>
) {
  if let Some(offered) = &prompt.0 {
    verb.set_if_neq(Text(offered.verb.clone()));
    noun.set_if_neq(Text(offered.noun.clone()));
  }
  fade.0 = approach(fade.0, prompt.0.is_some() as u8 as f32, time.delta_secs() * 7.0);
}

fn wane(
  time: Res<Time>,
  player: Single<&Vitals, With<Player>>,
  mut meters: Query<(&mut Meter, &mut Fade)>
) {
  let delta = time.delta_secs();
  meters.iter_mut().for_each(|(mut meter, mut fade)| {
    let now = meter.stat.of(&player);
    let settled = now > 0.995 && (now - meter.last).abs() < 1e-4;
    meter.idle = settled.then_some(meter.idle + delta).unwrap_or(0.0);
    meter.last = now;
    fade.0 = approach(fade.0, (meter.idle < 2.5) as u8 as f32, delta * 2.0);
  });
}

fn drain(
  time: Res<Time>,
  player: Single<&Vitals, With<Player>>,
  rival: Res<Rival>,
  mut fills: Query<(&mut Fill, &mut Node)>
) {
  let blend = 1.0 - (-9.0 * time.delta_secs()).exp();
  fills.iter_mut().for_each(|(mut fill, mut node)| {
    let want = match fill.gauge {
      Gauge::Own(stat) => stat.of(&player),
      Gauge::Rival => rival.fill
    };
    fill.shown = fill.shown.lerp(want, blend);
    node.left = Percent(50.0 * (1.0 - fill.shown));
    node.width = Percent(100.0 * fill.shown);
  });
}

fn swing_compass(
  time: Res<Time>,
  view: Res<View>,
  contact: Res<Contact>,
  player: Single<&Transform, With<Player>>,
  foes: Query<(Entity, &Transform, &Side, Option<&Foe>), Without<Dead>>,
  mut marks: Query<(&Heading, &mut Node, &mut Fade)>
) {
  let here = player.translation;
  let facing = -view.yaw;
  let now = time.elapsed_secs();
  let hostiles: Vec<Vec3> = foes
    .iter()
    .filter(|&(foe, transform, &side, mind)| {
      let hunting = mind.is_some_and(Foe::hunting);
      let fought = contact.0.get(&foe).is_some_and(|&at| now - at < 2.0 * FORGET);
      side == Side::Wild
        && transform.translation.distance(here) < HOSTILE_RANGE
        && (hunting || fought)
    })
    .map(|(_, transform, ..)| transform.translation)
    .take(DOTS)
    .collect();
  marks.iter_mut().for_each(|(&heading, mut node, mut fade)| {
    let target = match heading {
      Heading::Fixed(angle) => Some(angle),
      Heading::Toward(place) => {
        let gap = place.spot() - here.xz();
        (gap.length() < PLACE_RANGE && gap.length() > 1.0).then(|| bearing(gap))
      }
      Heading::Hostile(index) => hostiles.get(index).map(|at| bearing((*at - here).xz()))
    };
    let offset = target.map(|angle| (angle - facing + PI).rem_euclid(TAU) - PI);
    node.left = Percent(50.0 + offset.unwrap_or(0.0) / PI * 100.0);
    fade.0 =
      offset.map_or(0.0, |offset| smoothstep(FRAC_PI_2, FRAC_PI_2 * 0.78, offset.abs()));
  });
}

fn ink(charted: Res<Charted>, mut parts: Query<(&Ink, &mut Paint)>) {
  parts.iter_mut().for_each(|(ink, mut paint)| {
    let known = charted.0.contains(&ink.place);
    paint.back = Some(match (ink.stroke, known) {
      (_, true) => INK,
      (Stroke::Line, false) => DIM,
      (Stroke::Solid, false) => Color::NONE
    });
    paint.border = Some(BorderColor::all(known.then_some(INK).unwrap_or(DIM)));
  });
}

fn reticle(
  time: Res<Time>,
  view: Res<View>,
  mut fade: Single<&mut Fade, With<Crosshair>>
) {
  fade.0 = approach(fade.0, view.first_person as u8 as f32, time.delta_secs() * 6.0);
}

fn mourn(
  time: Res<Time>,
  player: Single<Has<Dead>, With<Player>>,
  mut fade: Single<&mut Fade, With<Mourning>>
) {
  let dead = *player;
  fade.0 = approach(
    fade.0,
    dead as u8 as f32,
    time.delta_secs() * dead.then_some(0.5).unwrap_or(0.8)
  );
}

fn unveil(
  time: Res<Time>,
  mut clock: Local<f32>,
  mut commands: Commands,
  intro: Single<(Entity, &mut Fade), With<Intro>>,
  mut titles: Query<&mut Fade, (With<IntroTitle>, Without<Intro>)>
) {
  *clock += time.delta_secs().min(1.0 / 30.0);
  let t = *clock;
  let (entity, mut veil) = intro.into_inner();
  veil.0 = 1.0 - smoothstep(4.3, INTRO_TIME - 0.2, t);
  titles.iter_mut().for_each(|mut title| {
    title.0 = envelope(t, [0.5, 1.8, 3.5, 4.7]);
  });
  if t > INTRO_TIME {
    commands.entity(entity).despawn();
  }
}

fn track(
  window: Single<&Window, With<PrimaryWindow>>,
  mut spaced: Query<(&Tracking, &mut LetterSpacing)>
) {
  let unit = window.width().min(window.height()) / 100.0;
  spaced.iter_mut().for_each(|(&Tracking(vmin), mut spacing)| {
    spacing.set_if_neq(LetterSpacing::Px(vmin * unit));
  });
}

fn remember(
  mut commands: Commands,
  huds: Query<(), With<Hud>>,
  parents: Query<&ChildOf>,
  fresh: Query<
    (
      Entity,
      Option<&TextColor>,
      Option<&BackgroundColor>,
      Option<&BorderColor>,
      Option<&BoxShadow>,
      Option<&TextShadow>,
      Option<&BackgroundGradient>
    ),
    (With<Node>, Without<Paint>)
  >
) {
  fresh
    .iter()
    .filter(|(entity, ..)| {
      parents.iter_ancestors(*entity).any(|above| huds.contains(above))
    })
    .for_each(|(entity, text, back, border, shadow, text_shadow, gradient)| {
      commands.entity(entity).insert(Paint {
        text: text.map(|color| color.0),
        back: back.map(|color| color.0),
        border: border.cloned(),
        shadow: shadow.cloned(),
        text_shadow: text_shadow.copied(),
        gradient: gradient.cloned()
      });
    });
}

fn tint(
  fades: Query<&Fade>,
  parents: Query<&ChildOf>,
  mut painted: Query<(
    Entity,
    &Paint,
    Option<&mut TextColor>,
    Option<&mut BackgroundColor>,
    Option<&mut BorderColor>,
    Option<&mut BoxShadow>,
    Option<&mut TextShadow>,
    Option<&mut BackgroundGradient>
  )>
) {
  painted.iter_mut().for_each(
    |(entity, paint, text, back, border, shadow, text_shadow, gradient)| {
      let alpha: f32 = std::iter::once(entity)
        .chain(parents.iter_ancestors(entity))
        .filter_map(|above| fades.get(above).ok())
        .map(|fade| fade.0)
        .product();
      let dim = |color: Color| color.with_alpha(color.alpha() * alpha);
      let stops = |stops: &[ColorStop]| {
        stops
          .iter()
          .map(|&stop| ColorStop { color: dim(stop.color), ..stop })
          .collect::<Vec<_>>()
      };
      if let Some(mut text) = text
        && let Some(color) = paint.text
      {
        text.set_if_neq(TextColor(dim(color)));
      }
      if let Some(mut back) = back
        && let Some(color) = paint.back
      {
        back.set_if_neq(BackgroundColor(dim(color)));
      }
      if let Some(mut border) = border
        && let Some(&BorderColor { top, right, bottom, left }) = paint.border.as_ref()
      {
        border.set_if_neq(BorderColor {
          top: dim(top),
          right: dim(right),
          bottom: dim(bottom),
          left: dim(left)
        });
      }
      if let Some(mut shadow) = shadow
        && let Some(base) = &paint.shadow
      {
        shadow.set_if_neq(BoxShadow(
          base
            .0
            .iter()
            .map(|&style| ShadowStyle { color: dim(style.color), ..style })
            .collect()
        ));
      }
      if let Some(mut text_shadow) = text_shadow
        && let Some(base) = paint.text_shadow
      {
        text_shadow.set_if_neq(TextShadow { color: dim(base.color), ..base });
      }
      if let Some(mut gradient) = gradient
        && let Some(base) = &paint.gradient
      {
        gradient.set_if_neq(BackgroundGradient(
          base
            .0
            .iter()
            .map(|layer| match layer {
              Gradient::Linear(linear) => Gradient::Linear(LinearGradient {
                stops: stops(&linear.stops),
                ..linear.clone()
              }),
              Gradient::Radial(radial) => Gradient::Radial(RadialGradient {
                stops: stops(&radial.stops),
                ..radial.clone()
              }),
              other => other.clone()
            })
            .collect()
        ));
      }
    }
  );
}

pub fn plugin(app: &mut App) {
  app
    .init_resource::<Charted>()
    .init_resource::<Contact>()
    .init_resource::<Rival>()
    .add_systems(Startup, raise)
    .add_systems(
      PostUpdate,
      (
        (engage, rival).chain(),
        (discover, herald).chain(),
        post,
        offer,
        wane,
        drain,
        swing_compass,
        ink,
        reticle,
        mourn,
        unveil,
        track,
        remember,
        tint
      )
        .chain()
        .before(UiSystems::Prepare)
    );
}
