//! STIX 2.1 object model: values, objects, bundles, and an object store.

pub mod error;
pub mod value;
pub mod view;

pub use error::{ModelError, Result};
pub use value::StixValue;
pub use view::{GenericObject, ObjectView};
