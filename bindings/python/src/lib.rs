//! Python bindings for the stix-rust toolkit (compiled module `stix._stix`).
use pyo3::prelude::*;

#[pymodule]
fn _stix(_m: &Bound<'_, PyModule>) -> PyResult<()> {
    Ok(())
}
