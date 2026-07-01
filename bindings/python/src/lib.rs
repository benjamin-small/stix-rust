//! Python bindings for the stix-rust toolkit (compiled module `stix._stix`).
use pyo3::prelude::*;

mod errors;

#[pymodule]
fn _stix(m: &Bound<'_, PyModule>) -> PyResult<()> {
    errors::register(m)?;
    Ok(())
}
