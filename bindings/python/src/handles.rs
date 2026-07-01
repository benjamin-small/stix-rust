//! Pyclass handles: Pattern, Bundle, MatchResult.
use pyo3::prelude::*;
use pyo3::types::PyList;

/// Parse a JSON string into a native Python object (dict/list/...).
fn json_to_py(py: Python<'_>, json: &str) -> PyResult<Py<PyAny>> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| crate::errors::ParseError::new_err(e.to_string()))?;
    let obj = pythonize::pythonize(py, &value)
        .map_err(|e| crate::errors::ModelError::new_err(e.to_string()))?;
    Ok(obj.unbind())
}

#[pyclass]
pub struct Pattern {
    pub(crate) inner: stix_ffi::Pattern,
}

#[pymethods]
impl Pattern {
    /// The pattern's AST as a native Python dict.
    #[getter]
    fn ast(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        json_to_py(py, &self.inner.to_json())
    }

    fn __repr__(&self) -> String {
        "Pattern(...)".to_string()
    }
}

#[pyclass]
pub struct Bundle {
    pub(crate) inner: stix_ffi::Bundle,
}

#[pymethods]
impl Bundle {
    /// Number of objects in the bundle.
    fn object_count(&self) -> usize {
        self.inner.object_count()
    }

    fn __len__(&self) -> usize {
        self.inner.object_count()
    }

    /// The object at `index` as a native Python dict, or None if out of range.
    fn object(&self, py: Python<'_>, index: usize) -> PyResult<Option<Py<PyAny>>> {
        match self.inner.object_json(index) {
            Some(json) => Ok(Some(json_to_py(py, &json)?)),
            None => Ok(None),
        }
    }

    /// Iterate over the objects (each a dict).
    fn __iter__(slf: PyRef<'_, Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let mut items: Vec<Py<PyAny>> = Vec::with_capacity(slf.inner.object_count());
        for i in 0..slf.inner.object_count() {
            if let Some(json) = slf.inner.object_json(i) {
                items.push(json_to_py(py, &json)?);
            }
        }
        let list = PyList::new(py, items)?;
        Ok(list.as_any().try_iter()?.into())
    }
}

#[pyclass]
pub struct MatchResult {
    pub(crate) matched: bool,
    pub(crate) observations: Vec<u64>,
}

#[pymethods]
impl MatchResult {
    #[getter]
    fn matched(&self) -> bool {
        self.matched
    }

    #[getter]
    fn observations(&self) -> Vec<u64> {
        self.observations.clone()
    }

    fn __bool__(&self) -> bool {
        self.matched
    }
}
