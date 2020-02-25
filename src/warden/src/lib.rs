//! Data-driven reference test framework for warding
//! against breaking changes.

#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

pub mod gpu;
pub mod raw;

#[derive(Debug, serde::Deserialize)]
pub enum Feature {}

impl Feature {
    pub fn into_hal(self) -> hal::Features {
        match self {}
    }
}
