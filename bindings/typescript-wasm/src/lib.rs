//! WebAssembly bindings for the stix-rust toolkit (raw wasm-bindgen layer).
//!
//! Errors are thrown as `"[code] message"`; the TypeScript wrapper maps the code
//! prefix onto the StixError subclass hierarchy.
use serde::Serialize;
use wasm_bindgen::prelude::*;

fn err_js(e: stix_ffi::FfiError) -> JsValue {
    let code = match e.code {
        stix_ffi::ErrorCode::Parse => "parse",
        stix_ffi::ErrorCode::Model => "model",
        stix_ffi::ErrorCode::Match => "match",
        stix_ffi::ErrorCode::Validation => "validation",
    };
    JsError::new(&format!("[{code}] {}", e.message)).into()
}

fn json_to_js(json: &str) -> Result<JsValue, JsValue> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| JsError::new(&format!("[model] {e}")))?;
    // Use the JSON-compatible serializer so JSON objects cross as plain JS objects
    // (property access, `JSON.stringify`) rather than as ES `Map` instances.
    let serializer = serde_wasm_bindgen::Serializer::json_compatible();
    value
        .serialize(&serializer)
        .map_err(|e| JsError::new(&e.to_string()).into())
}

#[wasm_bindgen]
pub struct Pattern {
    inner: stix_ffi::Pattern,
}

#[wasm_bindgen]
impl Pattern {
    #[wasm_bindgen(getter)]
    pub fn ast(&self) -> Result<JsValue, JsValue> {
        json_to_js(&self.inner.to_json())
    }
}

#[wasm_bindgen]
pub struct Bundle {
    inner: stix_ffi::Bundle,
}

#[wasm_bindgen]
impl Bundle {
    #[wasm_bindgen(js_name = objectCount)]
    pub fn object_count(&self) -> u32 {
        self.inner.object_count() as u32
    }

    #[wasm_bindgen]
    pub fn object(&self, index: u32) -> Result<JsValue, JsValue> {
        match self.inner.object_json(index as usize) {
            Some(json) => json_to_js(&json),
            None => Ok(JsValue::UNDEFINED),
        }
    }
}

#[wasm_bindgen]
pub struct MatchResult {
    matched: bool,
    observations: Vec<u32>,
}

#[wasm_bindgen]
impl MatchResult {
    #[wasm_bindgen(getter)]
    pub fn matched(&self) -> bool {
        self.matched
    }

    #[wasm_bindgen(getter)]
    pub fn observations(&self) -> Vec<u32> {
        self.observations.clone()
    }
}

#[wasm_bindgen]
pub struct Engine {
    inner: stix_ffi::Engine,
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Engine {
            inner: stix_ffi::Engine::new(),
        }
    }

    #[wasm_bindgen(js_name = parsePattern)]
    pub fn parse_pattern(&self, src: String) -> Result<Pattern, JsValue> {
        self.inner
            .parse_pattern(&src)
            .map(|inner| Pattern { inner })
            .map_err(err_js)
    }

    #[wasm_bindgen(js_name = parseBundle)]
    pub fn parse_bundle(&self, json: String) -> Result<Bundle, JsValue> {
        self.inner
            .parse_bundle(&json)
            .map(|inner| Bundle { inner })
            .map_err(err_js)
    }

    #[wasm_bindgen(js_name = matchBundle)]
    pub fn match_bundle(&self, pattern: &Pattern, bundle: &Bundle) -> Result<MatchResult, JsValue> {
        self.inner
            .match_bundle(&pattern.inner, &bundle.inner)
            .map(|o| MatchResult {
                matched: o.matched,
                observations: o.observations.iter().map(|&i| i as u32).collect(),
            })
            .map_err(err_js)
    }
}
