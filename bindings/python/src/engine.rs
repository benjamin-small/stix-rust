//! The Engine pyclass: parse patterns/bundles and run matches.
use pyo3::prelude::*;

use crate::errors::to_pyerr;
use crate::handles::{Bundle, MatchResult, Pattern};

#[pyclass]
pub struct Engine {
    inner: stix_ffi::Engine,
}

#[pymethods]
impl Engine {
    #[new]
    fn new() -> Self {
        Engine {
            inner: stix_ffi::Engine::new(),
        }
    }

    /// Parse a STIX pattern string into a Pattern handle.
    fn parse_pattern(&self, src: &str) -> PyResult<Pattern> {
        let inner = self.inner.parse_pattern(src).map_err(to_pyerr)?;
        Ok(Pattern { inner })
    }

    /// Parse a STIX bundle JSON string into a Bundle handle.
    fn parse_bundle(&self, json: &str) -> PyResult<Bundle> {
        let inner = self.inner.parse_bundle(json).map_err(to_pyerr)?;
        Ok(Bundle { inner })
    }

    /// Match a pattern against a bundle.
    fn match_bundle(&self, pattern: &Pattern, bundle: &Bundle) -> PyResult<MatchResult> {
        let outcome = self
            .inner
            .match_bundle(&pattern.inner, &bundle.inner)
            .map_err(to_pyerr)?;
        Ok(MatchResult {
            matched: outcome.matched,
            observations: outcome.observations,
        })
    }

    /// Register a custom object type. `hook` is a callable `dict -> dict`: it may
    /// mutate/return the object (adding computed properties) or raise to reject it
    /// (surfaced as ValidationError from parse_bundle). Runs only at parse time.
    fn register_type(&mut self, type_name: &str, hook: Py<PyAny>) {
        self.inner.register_type(
            type_name,
            Box::new(move |value: serde_json::Value| {
                Python::attach(|py| {
                    // Value -> Python dict
                    let arg = pythonize::pythonize(py, &value).map_err(|e| e.to_string())?;
                    // call the Python hook
                    let result = hook
                        .call1(py, (arg,))
                        .map_err(|e| e.value(py).to_string())?;
                    // returned dict -> Value
                    let out: serde_json::Value =
                        pythonize::depythonize(result.bind(py)).map_err(|e| e.to_string())?;
                    Ok(out)
                })
            }),
        );
    }
}
