use {crate::{combat::Vitals,
             hud::{self, DIM, FRAME, Fonts, Hud, INK, PALE},
             player::{Player, View}},
     bevy::{input::mouse::AccumulatedMouseScroll,
            prelude::*,
            text::{FontSize, FontSource, FontStyle},
            ui::{UiSystems,
                 Val::{Percent, Px, VMin, Vw}}},
     enum_assoc::Assoc,
     std::{collections::BTreeMap, fmt}};

const CARRY_MAX: f32 = 300.0;
const HIGHLIGHT: Color = Color::srgba(0.95, 0.94, 0.9, 0.13);

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, Assoc)]
#[func(pub const fn label(self) -> &'static str)]
pub enum Kind {
  #[default]
  #[assoc(label = "WEAPONS")]
  Weapons,
  #[assoc(label = "APPAREL")]
  Apparel,
  #[assoc(label = "POTIONS")]
  Potions,
  #[assoc(label = "MISC")]
  Misc
}

impl Kind {
  pub const ALL: [Kind; 4] = [Kind::Weapons, Kind::Apparel, Kind::Potions, Kind::Misc];
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Power {
  Damage(u32),
  Armor(u32),
  Heal(f32),
  Plain
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Assoc)]
#[func(pub const fn name(self) -> &'static str)]
#[func(pub const fn kind(self) -> Kind)]
#[func(pub const fn value(self) -> u32)]
#[func(pub const fn weight(self) -> f32)]
#[func(pub const fn power(self) -> Power { Power::Plain })]
pub enum Item {
  #[assoc(name = "Steel Sword", kind = Kind::Weapons, value = 45, weight = 10.0, power = Power::Damage(8))]
  SteelSword,
  #[assoc(name = "Iron Dagger", kind = Kind::Weapons, value = 10, weight = 2.0, power = Power::Damage(4))]
  IronDagger,
  #[assoc(name = "Barrow War Axe", kind = Kind::Weapons, value = 35, weight = 12.0, power = Power::Damage(8))]
  BarrowWarAxe,
  #[assoc(name = "Steel Battle Axe", kind = Kind::Weapons, value = 55, weight = 13.0, power = Power::Damage(9))]
  SteelBattleAxe,
  #[assoc(name = "Horned Iron Helmet", kind = Kind::Apparel, value = 60, weight = 5.0, power = Power::Armor(15))]
  HornedIronHelmet,
  #[assoc(name = "Iron Armor", kind = Kind::Apparel, value = 125, weight = 30.0, power = Power::Armor(25))]
  IronArmor,
  #[assoc(name = "Rimmed Iron Shield", kind = Kind::Apparel, value = 100, weight = 12.0, power = Power::Armor(22))]
  RimmedIronShield,
  #[assoc(name = "Fur Armor", kind = Kind::Apparel, value = 55, weight = 6.0, power = Power::Armor(23))]
  FurArmor,
  #[assoc(name = "Barrow Helm", kind = Kind::Apparel, value = 35, weight = 4.0, power = Power::Armor(16))]
  BarrowHelm,
  #[assoc(name = "Small Healing Draught", kind = Kind::Potions, value = 17, weight = 0.5, power = Power::Heal(25.0))]
  SmallHealingDraught,
  #[assoc(name = "Wolf Pelt", kind = Kind::Misc, value = 25, weight = 2.0)]
  WolfPelt,
  #[assoc(name = "Amethyst", kind = Kind::Misc, value = 120, weight = 0.1)]
  Amethyst,
  #[assoc(name = "Lockpick", kind = Kind::Misc, value = 3, weight = 0.0)]
  Lockpick,
  #[assoc(name = "Wight Lord's Key", kind = Kind::Misc, value = 0, weight = 0.0)]
  OverlordsKey,
  #[assoc(name = "Note: Rotfen Plans", kind = Kind::Misc, value = 0, weight = 0.0)]
  RotfenPlans,
  #[assoc(name = "Rime Crystal", kind = Kind::Misc, value = 100, weight = 0.25)]
  RimeCrystal
}

impl Item {
  pub const WORN: [Item; 4] =
    [Item::SteelSword, Item::RimmedIronShield, Item::HornedIronHelmet, Item::IronArmor];
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Loot {
  Gold(u32),
  Goods(Item, u32)
}

impl Loot {
  pub const fn one(item: Item) -> Self { Loot::Goods(item, 1) }
}

impl fmt::Display for Loot {
  fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
    match *self {
      Loot::Gold(count) => write!(out, "Gold ({count})"),
      Loot::Goods(item, 1) => write!(out, "{}", item.name()),
      Loot::Goods(item, count) => write!(out, "{} ({count})", item.name())
    }
  }
}

#[derive(Component)]
pub struct Inventory {
  pub gold: u32,
  goods: BTreeMap<Item, u32>
}

impl Inventory {
  fn outfitted() -> Self {
    Self { gold: 0, goods: Item::WORN.into_iter().map(|item| (item, 1)).collect() }
  }

  pub fn take(&mut self, loot: Loot) {
    match loot {
      Loot::Gold(count) => self.gold += count,
      Loot::Goods(item, count) => *self.goods.entry(item).or_default() += count
    }
  }

  pub fn holding(&self, item: Item) -> u32 { self.goods.get(&item).copied().unwrap_or(0) }

  pub fn spend(&mut self, item: Item) -> bool {
    let held = self.goods.remove(&item).unwrap_or(0);
    (held > 1).then(|| self.goods.insert(item, held - 1));
    held > 0
  }

  fn burden(&self) -> f32 {
    let total: f32 =
      self.goods.iter().map(|(item, &count)| item.weight() * count as f32).sum();
    (total * 10.0).round() / 10.0
  }

  fn shelf(&self, kind: Kind) -> Vec<(Item, u32)> {
    let mut shelf: Vec<(Item, u32)> = self
      .goods
      .iter()
      .filter(|(item, _)| item.kind() == kind)
      .map(|(&item, &count)| (item, count))
      .collect();
    shelf.sort_by_key(|(item, _)| item.name());
    shelf
  }
}

fn drink(item: Item, mut inventory: Mut<Inventory>, mut vitals: Mut<Vitals>) {
  if let Power::Heal(amount) = item.power()
    && inventory.spend(item)
  {
    vitals.health = (vitals.health + amount).min(vitals.health_max);
  }
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
enum Column {
  #[default]
  Kinds,
  Goods
}

#[derive(Resource, Clone, Copy, PartialEq, Default, Debug)]
pub struct Menu {
  open: bool,
  kind: Kind,
  row: usize,
  column: Column
}

#[derive(Component)]
struct Pack;

fn outfit(mut commands: Commands, player: Single<Entity, Added<Player>>) {
  commands.entity(*player).insert(Inventory::outfitted());
}

pub fn browse(
  mut keys: ResMut<ButtonInput<KeyCode>>,
  mut mouse: ResMut<ButtonInput<MouseButton>>,
  mut scroll: ResMut<AccumulatedMouseScroll>,
  mut menu: ResMut<Menu>,
  mut view: ResMut<View>,
  player: Single<(Mut<Inventory>, Mut<Vitals>), With<Player>>
) {
  let hit = |codes: [KeyCode; 2]| keys.any_just_pressed(codes);
  let toggled = hit([KeyCode::Tab, KeyCode::KeyI])
    || (menu.open && keys.just_pressed(KeyCode::Escape));
  let open = menu.open != toggled;
  let rise = hit([KeyCode::KeyS, KeyCode::ArrowDown]) as i32
    - hit([KeyCode::KeyW, KeyCode::ArrowUp]) as i32;
  let across = hit([KeyCode::KeyD, KeyCode::ArrowRight]) as i32
    - hit([KeyCode::KeyA, KeyCode::ArrowLeft]) as i32;
  let using = hit([KeyCode::Enter, KeyCode::KeyE]);
  let (inventory, vitals) = player.into_inner();
  let Menu { kind, row, column, .. } = *menu;
  let shelf = inventory.shelf(kind);
  let next = match (open, column) {
    (false, _) => Menu { open, ..*menu },
    (true, Column::Kinds) => {
      let kind =
        Kind::ALL[(kind as i32 + rise).clamp(0, Kind::ALL.len() as i32 - 1) as usize];
      Menu {
        open,
        kind,
        row: (kind == menu.kind).then_some(row).unwrap_or(0),
        column: (across > 0 || using).then_some(Column::Goods).unwrap_or(Column::Kinds)
      }
    }
    (true, Column::Goods) => Menu {
      open,
      kind,
      row: (row as i32 + rise).clamp(0, (shelf.len() as i32 - 1).max(0)) as usize,
      column: (across < 0).then_some(Column::Kinds).unwrap_or(Column::Goods)
    }
  };
  if open
    && using
    && column == Column::Goods
    && let Some(&(item, _)) = shelf.get(row)
  {
    drink(item, inventory, vitals);
  }
  if toggled {
    view.captured = !open;
  }
  if open || toggled {
    keys.reset_all();
    mouse.reset_all();
    scroll.delta = Vec2::ZERO;
  }
  menu.set_if_neq(next);
}

fn raise(mut commands: Commands) {
  commands.spawn((
    Name::new("Inventory"),
    Pack,
    Node {
      position_type: PositionType::Absolute,
      width: Percent(100.0),
      height: Percent(100.0),
      display: Display::None,
      ..default()
    },
    BackgroundGradient::from(LinearGradient::to_right(vec![
      ColorStop::new(Color::srgba(0.0, 0.0, 0.0, 0.92), Percent(0.0)),
      ColorStop::new(Color::srgba(0.01, 0.01, 0.01, 0.84), Percent(55.0)),
      ColorStop::new(Color::srgba(0.02, 0.02, 0.02, 0.7), Percent(100.0)),
    ])),
    GlobalZIndex(100)
  ));
}

fn rule() -> impl Bundle {
  (
    Node { width: Percent(100.0), height: Px(1.0), ..default() },
    hud::fading_line(true, FRAME)
  )
}

fn divider() -> impl Bundle {
  (
    Node { width: Px(1.0), height: Percent(100.0), ..default() },
    hud::fading_line(false, FRAME)
  )
}

fn lit(chosen: bool) -> BackgroundGradient {
  BackgroundGradient::from(LinearGradient::to_right(vec![
    ColorStop::new(chosen.then_some(HIGHLIGHT).unwrap_or(Color::NONE), Percent(0.0)),
    ColorStop::new(Color::NONE, Percent(100.0)),
  ]))
}

fn legend(parent: &mut ChildSpawnerCommands, fonts: &Fonts, cap: &str, verb: &str) {
  parent
    .spawn(Node { align_items: AlignItems::Center, column_gap: VMin(0.9), ..default() })
    .with_children(|hint| {
      hint
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
          key.spawn(hud::words(&fonts.sans, 1.5, INK, cap));
        });
      hint.spawn(hud::words(&fonts.light, 1.8, PALE, verb));
    });
}

fn tally(parent: &mut ChildSpawnerCommands, fonts: &Fonts, label: &str, amount: String) {
  parent
    .spawn(Node { align_items: AlignItems::Baseline, column_gap: VMin(1.0), ..default() })
    .with_children(|pair| {
      pair.spawn((hud::words(&fonts.sans, 1.5, DIM, label), hud::spaced(0.3)));
      pair.spawn(hud::words(&fonts.sans, 2.4, INK, amount));
    });
}

fn kinds(column: &mut ChildSpawnerCommands, fonts: &Fonts, menu: Menu) {
  for kind in Kind::ALL {
    let chosen = kind == menu.kind;
    let tone = match (chosen, menu.column) {
      (true, Column::Kinds) => INK,
      (true, Column::Goods) => PALE,
      _ => DIM
    };
    let pick = move |menu: Menu| Menu {
      kind,
      row: (kind == menu.kind).then_some(menu.row).unwrap_or(0),
      column: Column::Kinds,
      ..menu
    };
    column
      .spawn((
        Node {
          align_items: AlignItems::Center,
          column_gap: VMin(1.4),
          padding: UiRect::axes(VMin(1.4), VMin(0.9)),
          ..default()
        },
        lit(chosen && menu.column == Column::Kinds)
      ))
      .observe(move |_: On<Pointer<Move>>, mut menu: ResMut<Menu>| {
        let next = pick(*menu);
        menu.set_if_neq(next);
      })
      .observe(move |_: On<Pointer<Click>>, mut menu: ResMut<Menu>| {
        let next = pick(*menu);
        menu.set_if_neq(next);
      })
      .with_children(|line| {
        line
          .spawn(Node { width: VMin(1.0), height: VMin(1.0), ..default() })
          .with_children(|slot| {
            chosen.then(|| slot.spawn(hud::diamond(0.8, hud::at(50.0, 50.0))));
          });
        line.spawn((hud::words(&fonts.sans, 2.2, tone, kind.label()), hud::spaced(0.35)));
      });
  }
}

fn goods(
  column: &mut ChildSpawnerCommands,
  fonts: &Fonts,
  menu: Menu,
  shelf: &[(Item, u32)]
) {
  if shelf.is_empty() {
    column
      .spawn(Node { padding: UiRect::axes(VMin(1.4), VMin(0.7)), ..default() })
      .with_children(|line| {
        line.spawn(hud::words(&fonts.light, 2.0, DIM, "Nothing carried"));
      });
  }
  for (index, &(item, count)) in shelf.iter().enumerate() {
    let chosen = index == menu.row;
    let tone = match (chosen, menu.column) {
      (true, Column::Goods) => INK,
      (true, Column::Kinds) => PALE,
      _ => PALE.with_alpha(0.7)
    };
    let label = (count > 1)
      .then(|| format!("{} ({count})", item.name()))
      .unwrap_or(item.name().into());
    column
      .spawn((
        Node {
          align_items: AlignItems::Center,
          column_gap: VMin(1.2),
          padding: UiRect::axes(VMin(1.2), VMin(0.7)),
          ..default()
        },
        lit(chosen && menu.column == Column::Goods)
      ))
      .observe(move |_: On<Pointer<Move>>, mut menu: ResMut<Menu>| {
        menu.set_if_neq(Menu { row: index, column: Column::Goods, ..*menu });
      })
      .observe(
        move |_: On<Pointer<Click>>,
              mut menu: ResMut<Menu>,
              player: Single<(Mut<Inventory>, Mut<Vitals>), With<Player>>| {
          menu.set_if_neq(Menu { row: index, column: Column::Goods, ..*menu });
          let (inventory, vitals) = player.into_inner();
          drink(item, inventory, vitals);
        }
      )
      .with_children(|line| {
        line
          .spawn(Node { width: VMin(1.0), height: VMin(1.0), ..default() })
          .with_children(|slot| {
            Item::WORN
              .contains(&item)
              .then(|| slot.spawn(hud::diamond(0.75, hud::at(50.0, 50.0))));
          });
        line.spawn(hud::words(&fonts.sans, 2.1, tone, label));
      });
  }
}

fn details(column: &mut ChildSpawnerCommands, fonts: &Fonts, item: Item) {
  column.spawn((
    hud::words(&fonts.serif, 3.6, INK, item.name()),
    TextLayout::justify(Justify::Center)
  ));
  column.spawn((
    Node { width: Percent(80.0), height: Px(1.0), ..default() },
    hud::fading_line(true, PALE)
  ));
  column
    .spawn(Node { column_gap: VMin(3.6), align_items: AlignItems::Baseline, ..default() })
    .with_children(|stats| {
      match item.power() {
        Power::Damage(damage) => tally(stats, fonts, "DAMAGE", damage.to_string()),
        Power::Armor(armor) => tally(stats, fonts, "ARMOR", armor.to_string()),
        Power::Heal(_) | Power::Plain => {}
      }
      tally(stats, fonts, "WEIGHT", item.weight().to_string());
      tally(stats, fonts, "VALUE", item.value().to_string());
    });
  if let Power::Heal(amount) = item.power() {
    column.spawn((
      Text::new(format!("Heals {amount} health.")),
      TextColor(PALE),
      TextFont {
        font: FontSource::Handle(fonts.light.clone()),
        font_size: FontSize::VMin(2.0),
        style: FontStyle::Italic,
        ..default()
      }
    ));
  }
  if Item::WORN.contains(&item) {
    column.spawn((hud::words(&fonts.sans, 1.5, DIM, "EQUIPPED"), hud::spaced(0.45)));
  }
}

fn sheet(
  pack: &mut ChildSpawnerCommands,
  fonts: &Fonts,
  menu: Menu,
  inventory: &Inventory
) {
  let shelf = inventory.shelf(menu.kind);
  let chosen = shelf.get(menu.row).map(|&(item, _)| item);
  pack
    .spawn(Node {
      width: Percent(100.0),
      height: Percent(100.0),
      flex_direction: FlexDirection::Column,
      padding: UiRect::axes(Vw(6.0), VMin(5.0)),
      row_gap: VMin(1.6),
      ..default()
    })
    .with_children(|sheet| {
      sheet
        .spawn(Node {
          flex_direction: FlexDirection::Column,
          align_items: AlignItems::Center,
          row_gap: VMin(1.4),
          ..default()
        })
        .with_children(|head| {
          head.spawn((hud::words(&fonts.serif, 3.4, INK, "INVENTORY"), hud::spaced(0.8)));
          head.spawn(rule());
        });
      sheet
        .spawn(Node {
          flex_grow: 1.0,
          min_height: Px(0.0),
          column_gap: VMin(3.0),
          ..default()
        })
        .with_children(|body| {
          body
            .spawn(Node {
              width: Percent(20.0),
              flex_shrink: 0.0,
              flex_direction: FlexDirection::Column,
              row_gap: VMin(0.8),
              padding: UiRect::top(VMin(2.0)),
              ..default()
            })
            .with_children(|column| kinds(column, fonts, menu));
          body.spawn(divider());
          body
            .spawn(Node {
              width: Percent(34.0),
              flex_shrink: 0.0,
              flex_direction: FlexDirection::Column,
              row_gap: VMin(0.3),
              padding: UiRect::top(VMin(2.0)),
              ..default()
            })
            .with_children(|column| goods(column, fonts, menu, &shelf));
          body.spawn(divider());
          body
            .spawn(Node {
              flex_grow: 1.0,
              min_width: Px(0.0),
              flex_direction: FlexDirection::Column,
              align_items: AlignItems::Center,
              row_gap: VMin(2.0),
              padding: UiRect::top(VMin(12.0)),
              ..default()
            })
            .with_children(|column| {
              chosen.map(|item| details(column, fonts, item));
            });
        });
      sheet.spawn(rule());
      sheet
        .spawn(Node {
          justify_content: JustifyContent::SpaceBetween,
          align_items: AlignItems::Center,
          padding: UiRect::axes(VMin(1.4), VMin(0.4)),
          ..default()
        })
        .with_children(|foot| {
          foot.spawn(Node { column_gap: VMin(3.0), ..default() }).with_children(
            |hints| {
              chosen
                .filter(|item| matches!(item.power(), Power::Heal(_)))
                .map(|_| legend(hints, fonts, "E", "Use"));
              legend(hints, fonts, "Tab", "Close");
            }
          );
          foot
            .spawn(Node {
              column_gap: VMin(4.5),
              align_items: AlignItems::Baseline,
              ..default()
            })
            .with_children(|totals| {
              tally(
                totals,
                fonts,
                "WEIGHT",
                format!("{} / {CARRY_MAX}", inventory.burden())
              );
              tally(totals, fonts, "GOLD", inventory.gold.to_string());
            });
        });
    });
}

fn draw(
  mut commands: Commands,
  menu: Res<Menu>,
  fonts: Res<Fonts>,
  player: Single<Ref<Inventory>, With<Player>>,
  pack: Single<(Entity, &mut Node), With<Pack>>,
  mut hud: Single<&mut Visibility, With<Hud>>
) {
  let inventory = player.into_inner();
  if menu.is_changed() || inventory.is_changed() {
    let (entity, mut node) = pack.into_inner();
    node.display = menu.open.then_some(Display::Flex).unwrap_or(Display::None);
    **hud = menu.open.then_some(Visibility::Hidden).unwrap_or(Visibility::Inherited);
    commands.entity(entity).despawn_children().with_children(|pack| {
      if menu.open {
        sheet(pack, &fonts, *menu, &inventory);
      }
    });
  }
}

pub fn plugin(app: &mut App) {
  app
    .init_resource::<Menu>()
    .add_systems(Startup, raise)
    .add_systems(PostStartup, outfit)
    .add_systems(PreUpdate, browse.after(bevy::input::InputSystems))
    .add_systems(PostUpdate, draw.before(UiSystems::Prepare));
}
