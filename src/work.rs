use {bevy::prelude::*, std::sync::atomic::Ordering};

#[cfg(target_arch = "wasm32")]
const BUDGET_MICROS: u64 = 6000;
#[cfg(not(target_arch = "wasm32"))]
const HANDOVERS: usize = 12;

#[cfg(target_arch = "wasm32")]
static SPENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[cfg(not(target_arch = "wasm32"))]
static HANDED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn unlimited() -> bool { crate::opts::opts().shot.is_some() }

#[cfg(not(target_arch = "wasm32"))]
fn hand_over() -> bool {
  let handed = HANDED.fetch_add(1, Ordering::Relaxed);
  unlimited() || handed < HANDOVERS
}

#[cfg(target_arch = "wasm32")]
fn room() -> bool {
  let spent = SPENT.load(Ordering::Relaxed);
  unlimited() || spent == 0 || spent < BUDGET_MICROS
}

#[cfg(target_arch = "wasm32")]
fn spend(start: web_time::Instant) {
  SPENT.fetch_add(start.elapsed().as_micros() as u64 + 1, Ordering::Relaxed);
}

pub struct Work<Made>(Box<dyn FnMut() -> Option<Made> + Send>);

pub fn once<Made: Send + 'static>(
  make: impl FnOnce() -> Made + Send + 'static
) -> Work<Made> {
  let mut make = Some(make);
  Work(Box::new(move || make.take().map(|make| make())))
}

pub fn rows<Part: Send + 'static>(
  count: usize,
  row: impl Fn(usize) -> Part + Send + 'static
) -> Work<Vec<Part>> {
  let mut parts = Vec::with_capacity(count);
  Work(Box::new(move || {
    (parts.len() < count).then(|| parts.push(row(parts.len())));
    (parts.len() == count).then(|| std::mem::take(&mut parts))
  }))
}

impl<Made: Send + 'static> Work<Made> {
  fn step(&mut self) -> Option<Made> { (self.0)() }

  pub fn then<Next: Send + 'static>(
    mut self,
    next: impl FnOnce(Made) -> Work<Next> + Send + 'static
  ) -> Work<Next> {
    let mut next = Some(next);
    let mut second: Option<Work<Next>> = None;
    Work(Box::new(move || match second.as_mut() {
      Some(work) => work.step(),
      None => {
        second = self.step().and_then(|made| next.take().map(|next| next(made)));
        None
      }
    }))
  }

  pub fn map<Next: Send + 'static>(
    self,
    finish: impl FnOnce(Made) -> Next + Send + 'static
  ) -> Work<Next> {
    self.then(|made| once(move || finish(made)))
  }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct Job<Made>(bevy::tasks::Task<Made>);

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn<Made: Send + 'static>(work: Work<Made>) -> Job<Made> {
  Job(bevy::tasks::AsyncComputeTaskPool::get().spawn(async move { run(work) }))
}

#[cfg(not(target_arch = "wasm32"))]
impl<Made> Job<Made> {
  pub fn done(&mut self) -> Option<Made> {
    (self.0.is_finished() && hand_over())
      .then(|| bevy::tasks::futures::check_ready(&mut self.0))
      .flatten()
  }
}

#[cfg(target_arch = "wasm32")]
pub struct Job<Made>(std::sync::Mutex<Work<Made>>);

#[cfg(target_arch = "wasm32")]
pub fn spawn<Made: Send + 'static>(work: Work<Made>) -> Job<Made> {
  Job(std::sync::Mutex::new(work))
}

#[cfg(target_arch = "wasm32")]
impl<Made: Send + 'static> Job<Made> {
  pub fn done(&mut self) -> Option<Made> {
    let work = self.0.get_mut().expect("unpoisoned work");
    std::iter::from_fn(|| room().then_some(())).find_map(|()| {
      let start = web_time::Instant::now();
      let made = work.step();
      spend(start);
      made
    })
  }
}

pub fn task<Made: Send + 'static>(
  make: impl FnOnce() -> Made + Send + 'static
) -> Job<Made> {
  spawn(once(make))
}

pub fn run<Made: Send + 'static>(mut work: Work<Made>) -> Made {
  std::iter::repeat_with(|| work.step()).find_map(|made| made).expect("work finishes")
}

#[cfg(target_arch = "wasm32")]
fn open_budget() { SPENT.store(0, Ordering::Relaxed); }

#[cfg(not(target_arch = "wasm32"))]
fn open_budget() { HANDED.store(0, Ordering::Relaxed); }

pub fn plugin(app: &mut App) { app.add_systems(First, open_budget); }
