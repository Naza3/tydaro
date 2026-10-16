//! Test-only helpers. Native scenarios require the explicit test-support feature.
#[cfg(test)]
mod temp_dir;
#[cfg(test)]
pub(crate) use temp_dir::TestDirectory;

#[cfg(feature = "test-support")]
mod native;
#[cfg(feature = "test-support")]
pub(crate) use native::{run, run_architecture, run_desktop};
#[cfg(feature = "test-support")]
mod desktop;
#[cfg(feature = "test-support")]
pub(crate) mod gallery;
