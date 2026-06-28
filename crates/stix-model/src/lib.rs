//! STIX 2.1 object model: values, objects, bundles, and an object store.

pub mod error;
pub mod value;

pub use error::{ModelError, Result};
pub use value::StixValue;
