use {serde::Deserialize, std::sync::LazyLock};

const VAR: &str = "SKYRIM";

#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Opts {
  pub hour: f32,
  pub day: f32,
  pub shot: Option<f32>,
  pub at: Option<String>,
  pub yaw: Option<f32>,
  pub pitch: Option<f32>,
  pub intro: bool,
  pub zoom: Option<f32>,
  pub turn: f32,
  pub pose: Option<String>,
  pub foe: Option<String>,
  pub inside: Option<[f32; 3]>,
  pub first: bool,
  pub dragon: Option<f32>,
  pub press: Vec<(f32, String)>,
  pub eye: Option<[f32; 3]>,
  pub look: Option<[f32; 3]>,
  pub haze: f32
}

impl Default for Opts {
  fn default() -> Self {
    Self {
      hour: 9.5,
      day: 2400.0,
      shot: None,
      at: None,
      yaw: None,
      pitch: None,
      intro: true,
      zoom: None,
      turn: 0.0,
      pose: None,
      foe: None,
      inside: None,
      first: false,
      dragon: None,
      press: Vec::new(),
      eye: None,
      look: None,
      haze: 1.0
    }
  }
}

pub fn opts() -> &'static Opts {
  static OPTS: LazyLock<Opts> = LazyLock::new(|| {
    std::env::var(VAR).ok().filter(|text| !text.trim().is_empty()).map_or_else(
      Opts::default,
      |text| {
        json5::from_str(&text).unwrap_or_else(|blame| panic!("{VAR}={text}\n  {blame}"))
      }
    )
  });
  &OPTS
}
