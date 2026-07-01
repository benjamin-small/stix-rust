//! Native Node bindings for the stix-rust toolkit (raw napi layer).
//!
//! Errors are thrown as `"[code] message"`; the TypeScript wrapper maps the code
//! prefix onto the StixError subclass hierarchy.
#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;

fn map_err(e: stix_ffi::FfiError) -> Error {
    let code = match e.code {
        stix_ffi::ErrorCode::Parse => "parse",
        stix_ffi::ErrorCode::Model => "model",
        stix_ffi::ErrorCode::Match => "match",
        stix_ffi::ErrorCode::Validation => "validation",
    };
    Error::from_reason(format!("[{code}] {}", e.message))
}

fn json_err(e: serde_json::Error) -> Error {
    Error::from_reason(format!("[model] {e}"))
}

#[napi]
pub struct Pattern {
    inner: stix_ffi::Pattern,
}

#[napi]
impl Pattern {
    #[napi(getter)]
    pub fn ast(&self) -> Result<serde_json::Value> {
        serde_json::from_str(&self.inner.to_json()).map_err(json_err)
    }
}

#[napi]
pub struct Bundle {
    inner: stix_ffi::Bundle,
}

#[napi]
impl Bundle {
    #[napi]
    pub fn object_count(&self) -> u32 {
        self.inner.object_count() as u32
    }

    #[napi]
    pub fn object(&self, index: u32) -> Result<Option<serde_json::Value>> {
        match self.inner.object_json(index as usize) {
            Some(json) => Ok(Some(serde_json::from_str(&json).map_err(json_err)?)),
            None => Ok(None),
        }
    }
}

#[napi]
pub struct MatchResult {
    inner_matched: bool,
    inner_observations: Vec<u32>,
}

#[napi]
impl MatchResult {
    #[napi(getter)]
    pub fn matched(&self) -> bool {
        self.inner_matched
    }

    #[napi(getter)]
    pub fn observations(&self) -> Vec<u32> {
        self.inner_observations.clone()
    }
}

#[napi]
pub struct Engine {
    inner: stix_ffi::Engine,
}

#[napi]
impl Engine {
    #[napi(constructor)]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Engine {
            inner: stix_ffi::Engine::new(),
        }
    }

    #[napi]
    pub fn parse_pattern(&self, src: String) -> Result<Pattern> {
        self.inner
            .parse_pattern(&src)
            .map(|inner| Pattern { inner })
            .map_err(map_err)
    }

    #[napi]
    pub fn parse_bundle(&self, json: String) -> Result<Bundle> {
        self.inner
            .parse_bundle(&json)
            .map(|inner| Bundle { inner })
            .map_err(map_err)
    }

    #[napi]
    pub fn match_bundle(&self, pattern: &Pattern, bundle: &Bundle) -> Result<MatchResult> {
        self.inner
            .match_bundle(&pattern.inner, &bundle.inner)
            .map(|o| MatchResult {
                inner_matched: o.matched,
                inner_observations: o.observations.iter().map(|&i| i as u32).collect(),
            })
            .map_err(map_err)
    }
}
