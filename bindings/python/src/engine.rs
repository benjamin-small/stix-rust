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
}
