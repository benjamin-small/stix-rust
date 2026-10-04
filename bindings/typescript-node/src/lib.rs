//! Native Node bindings for the stix-rust toolkit (raw napi layer).
//!
//! Errors are thrown as `"[code] message"`; the TypeScript wrapper maps the code
//! prefix onto the StixError subclass hierarchy.
//!
//! # Handles
//!
//! Native objects cross the boundary as napi `External` values, never as
//! `#[napi]` classes. napi-rs 2.x class unwrapping (`FromNapiRef`) calls
//! `napi_unwrap` and casts the pointer to the requested type without checking
//! it, so passing a wrapped object of one class where another was expected
//! reinterpreted memory and crashed the process. An `External<T>` stores the
//! Rust `TypeId` of `T` next to the value and napi-rs compares it on every
//! unwrap, so a handle of the wrong type is an ordinary error here. Every
//! handle argument goes through [`handle`], which also maps any mismatch to a
//! `[validation]` error.
#![deny(clippy::all)]

use napi::bindgen_prelude::*;
#[allow(deprecated)]
use napi::JsExternal;
use napi::JsUnknown;
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

/// Parse an AST/IR JSON document with serde_json's default 128-level recursion
/// limit disabled. This is safe only because the document comes from
/// `Pattern::to_json()` on a pattern accepted by `parse`, which caps nesting at
/// `stix_pattern::MAX_NESTING` (40), so JSON depth stays at about
/// 2 * MAX_NESTING plus a small constant. Do not use it for untrusted JSON.
fn parse_ast_json(json: &str) -> serde_json::Result<serde_json::Value> {
    use serde::Deserialize;
    let mut de = serde_json::Deserializer::from_str(json);
    de.disable_recursion_limit();
    let value = serde_json::Value::deserialize(&mut de)?;
    de.end()?;
    Ok(value)
}

fn json_err(e: serde_json::Error) -> Error {
    Error::from_reason(format!("[model] {e}"))
}

/// Recover a `&T` from a JS value that must be an `External<T>` created by this
/// addon. Non-externals and externals holding another type are rejected with a
/// `[validation]` error naming the expected handle.
///
/// `JsExternal` / `Env::get_value_external` are deprecated in napi 2.16 in
/// favour of `External<T>`, but taking `External<T>` as a parameter type lets
/// the generated glue throw its own (code-less) error, and converting a
/// `JsUnknown` to `External<T>` by hand needs `unsafe`. This safe pair performs
/// the same value-type and TypeId checks.
#[allow(deprecated)]
fn handle<'env, T: 'static>(env: &'env Env, value: JsUnknown, what: &str) -> Result<&'env T> {
    let invalid = || Error::from_reason(format!("[validation] expected a handle of type {what}"));
    let external = JsExternal::try_from(value).map_err(|_| invalid())?;
    // `get_value_external` checks the stored TypeId against `T` before casting.
    // It hands out `&mut T`; we downgrade to `&T` at once. No entry point
    // requests the same `T` twice, so no two references alias one value.
    env.get_value_external::<T>(&external)
        .map(|r| &*r)
        .map_err(|_| invalid())
}

/// The outcome of a match, returned as a plain JS object.
#[napi(object)]
pub struct MatchOutcome {
    pub matched: bool,
    pub observations: Vec<u32>,
}

#[napi(ts_return_type = "ExternalObject<'Engine'>")]
pub fn create_engine() -> External<stix_ffi::Engine> {
    External::new(stix_ffi::Engine::new())
}

#[napi(ts_return_type = "ExternalObject<'Pattern'>")]
pub fn parse_pattern(
    env: Env,
    #[napi(ts_arg_type = "ExternalObject<'Engine'>")] engine: JsUnknown,
    src: String,
) -> Result<External<stix_ffi::Pattern>> {
    let engine = handle::<stix_ffi::Engine>(&env, engine, "Engine")?;
    engine
        .parse_pattern(&src)
        .map(External::new)
        .map_err(map_err)
}

#[napi(ts_return_type = "ExternalObject<'Bundle'>")]
pub fn parse_bundle(
    env: Env,
    #[napi(ts_arg_type = "ExternalObject<'Engine'>")] engine: JsUnknown,
    json: String,
) -> Result<External<stix_ffi::Bundle>> {
    let engine = handle::<stix_ffi::Engine>(&env, engine, "Engine")?;
    engine
        .parse_bundle(&json)
        .map(External::new)
        .map_err(map_err)
}

#[napi]
pub fn match_bundle(
    env: Env,
    #[napi(ts_arg_type = "ExternalObject<'Engine'>")] engine: JsUnknown,
    #[napi(ts_arg_type = "ExternalObject<'Pattern'>")] pattern: JsUnknown,
    #[napi(ts_arg_type = "ExternalObject<'Bundle'>")] bundle: JsUnknown,
) -> Result<MatchOutcome> {
    let engine = handle::<stix_ffi::Engine>(&env, engine, "Engine")?;
    let pattern = handle::<stix_ffi::Pattern>(&env, pattern, "Pattern")?;
    let bundle = handle::<stix_ffi::Bundle>(&env, bundle, "Bundle")?;
    engine
        .match_bundle(pattern, bundle)
        .map(|o| MatchOutcome {
            matched: o.matched,
            observations: o.observations.iter().map(|&i| i as u32).collect(),
        })
        .map_err(map_err)
}

#[napi]
pub fn pattern_ast(
    env: Env,
    #[napi(ts_arg_type = "ExternalObject<'Pattern'>")] pattern: JsUnknown,
) -> Result<serde_json::Value> {
    let pattern = handle::<stix_ffi::Pattern>(&env, pattern, "Pattern")?;
    parse_ast_json(&pattern.to_json()).map_err(json_err)
}

#[napi]
pub fn bundle_object_count(
    env: Env,
    #[napi(ts_arg_type = "ExternalObject<'Bundle'>")] bundle: JsUnknown,
) -> Result<u32> {
    let bundle = handle::<stix_ffi::Bundle>(&env, bundle, "Bundle")?;
    Ok(bundle.object_count() as u32)
}

#[napi]
pub fn bundle_object(
    env: Env,
    #[napi(ts_arg_type = "ExternalObject<'Bundle'>")] bundle: JsUnknown,
    index: u32,
) -> Result<Option<serde_json::Value>> {
    let bundle = handle::<stix_ffi::Bundle>(&env, bundle, "Bundle")?;
    match bundle.object_json(index as usize) {
        Some(json) => Ok(Some(serde_json::from_str(&json).map_err(json_err)?)),
        None => Ok(None),
    }
}
