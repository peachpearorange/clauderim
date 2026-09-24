use {crate::place::Place, bevy::prelude::*};

#[derive(Message, Clone)]
pub struct Notice(pub String);

#[derive(Message, Clone, Copy)]
pub struct Discovered(pub Place);

#[derive(Clone)]
pub struct Prompting {
  pub verb: String,
  pub noun: String
}

#[derive(Resource, Default)]
pub struct Prompt(pub Option<Prompting>);

#[derive(Resource, Default)]
pub struct Engaged(pub Option<Entity>);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Cue {
  Swing,
  PowerSwing,
  Hit,
  Block,
  Footstep,
  WolfGrowl,
  WolfBite,
  WolfDie,
  DraugrWake,
  DraugrGroan,
  DraugrDie,
  BanditShout,
  ManDie,
  PlayerHurt,
  ChestOpen,
  Discover,
  WordWall,
  WordLearned,
  Shout,
  DragonRoar,
  Wingbeat,
  LevelUp,
  Coins
}

#[derive(Message, Clone, Copy)]
pub struct Sound {
  pub cue: Cue,
  pub at: Option<Vec3>
}

impl Sound {
  pub fn here(cue: Cue, at: Vec3) -> Self { Self { cue, at: Some(at) } }

  pub fn flat(cue: Cue) -> Self { Self { cue, at: None } }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum FoeKind {
  Wolf,
  Draugr,
  DraugrOverlord,
  Bandit,
  BanditChief
}

#[derive(Component, Clone, Copy)]
pub struct FoeSpawn {
  pub kind: FoeKind,
  pub dormant: bool
}

#[derive(Component)]
pub struct WordWall;

#[derive(Resource, Default)]
pub struct Shouts {
  pub learned: u32,
  pub cooldown: f32
}

fn forget_prompt(mut prompt: ResMut<Prompt>) { prompt.0 = None; }

pub fn plugin(app: &mut App) {
  app
    .add_systems(First, forget_prompt)
    .add_message::<Notice>()
    .add_message::<Discovered>()
    .add_message::<Sound>()
    .init_resource::<Prompt>()
    .init_resource::<Engaged>()
    .init_resource::<Shouts>();
}
