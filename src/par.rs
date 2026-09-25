#[cfg(not(target_arch = "wasm32"))]
pub use std::thread::{scope, spawn};

#[cfg(target_arch = "wasm32")]
pub struct Scope;

#[cfg(target_arch = "wasm32")]
pub struct Handle<Made>(Made);

#[cfg(target_arch = "wasm32")]
impl<Made> Handle<Made> {
  pub fn join(self) -> std::thread::Result<Made> { Ok(self.0) }
}

#[cfg(target_arch = "wasm32")]
impl Scope {
  pub fn spawn<Made>(&self, work: impl FnOnce() -> Made) -> Handle<Made> {
    Handle(work())
  }
}

#[cfg(target_arch = "wasm32")]
pub fn scope<Made>(work: impl FnOnce(&Scope) -> Made) -> Made { work(&Scope) }

#[cfg(target_arch = "wasm32")]
pub fn spawn<Made>(work: impl FnOnce() -> Made) -> Handle<Made> { Handle(work()) }
