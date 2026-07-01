//! Python bindings for the stix-rust toolkit (compiled module `stix._stix`).
use pyo3::prelude::*;

mod engine;
mod errors;
mod handles;

#[pymodule]
fn _stix(m: &Bound<'_, PyModule>) -> PyResult<()> {
    errors::register(m)?;
    m.add_class::<engine::Engine>()?;
    m.add_class::<handles::Pattern>()?;
    m.add_class::<handles::Bundle>()?;
    m.add_class::<handles::MatchResult>()?;
    Ok(())
}
