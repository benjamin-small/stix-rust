//! Python exception types and the FfiError -> PyErr mapping.
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use stix_ffi::{ErrorCode, FfiError};

create_exception!(_stix, StixError, PyException, "Base class for all stix errors.");
create_exception!(_stix, ParseError, StixError, "Pattern failed to parse.");
create_exception!(_stix, ModelError, StixError, "Object/bundle import failed.");
create_exception!(_stix, MatchError, StixError, "Matching failed.");
create_exception!(
    _stix,
    ValidationError,
    StixError,
    "A custom-type hook rejected an object."
);

/// Convert a facade error into the matching Python exception.
pub fn to_pyerr(err: FfiError) -> PyErr {
    match err.code {
        ErrorCode::Parse => ParseError::new_err(err.message),
        ErrorCode::Model => ModelError::new_err(err.message),
        ErrorCode::Match => MatchError::new_err(err.message),
        ErrorCode::Validation => ValidationError::new_err(err.message),
    }
}

/// Register the exception types on the module.
pub fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("StixError", m.py().get_type::<StixError>())?;
    m.add("ParseError", m.py().get_type::<ParseError>())?;
    m.add("ModelError", m.py().get_type::<ModelError>())?;
    m.add("MatchError", m.py().get_type::<MatchError>())?;
    m.add("ValidationError", m.py().get_type::<ValidationError>())?;
    Ok(())
}
