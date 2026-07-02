//! JNI bindings for the stix-rust toolkit (raw layer).
//!
//! Handles are boxed pointers passed as jlong. Deep structure crosses as JSON
//! strings; the Java wrapper parses them with Jackson. On FfiError, the matching
//! Java exception is thrown.
use jni::objects::{JClass, JString};
use jni::sys::{jint, jlong, jstring};
use jni::JNIEnv;

fn throw(env: &mut JNIEnv, e: stix_ffi::FfiError) {
    let class = match e.code {
        stix_ffi::ErrorCode::Parse => "io/github/benjaminsmall/stix/ParseException",
        stix_ffi::ErrorCode::Model => "io/github/benjaminsmall/stix/ModelException",
        stix_ffi::ErrorCode::Match => "io/github/benjaminsmall/stix/MatchException",
        stix_ffi::ErrorCode::Validation => "io/github/benjaminsmall/stix/ValidationException",
    };
    let _ = env.throw_new(class, e.message);
}

fn read_string(env: &mut JNIEnv, s: &JString) -> String {
    env.get_string(s).map(|js| js.into()).unwrap_or_default()
}

fn out_string(env: &mut JNIEnv, s: String) -> jstring {
    env.new_string(s)
        .map(|js| js.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// --- Engine ---

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Engine_nativeNew<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jlong {
    Box::into_raw(Box::new(stix_ffi::Engine::new())) as jlong
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Engine_nativeFree<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
) {
    if ptr != 0 {
        unsafe { drop(Box::from_raw(ptr as *mut stix_ffi::Engine)) };
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Engine_nativeParsePattern<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    engine_ptr: jlong,
    src: JString<'l>,
) -> jlong {
    let engine = unsafe { &*(engine_ptr as *const stix_ffi::Engine) };
    let src = read_string(&mut env, &src);
    match engine.parse_pattern(&src) {
        Ok(p) => Box::into_raw(Box::new(p)) as jlong,
        Err(e) => {
            throw(&mut env, e);
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Engine_nativeParseBundle<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    engine_ptr: jlong,
    json: JString<'l>,
) -> jlong {
    let engine = unsafe { &*(engine_ptr as *const stix_ffi::Engine) };
    let json = read_string(&mut env, &json);
    match engine.parse_bundle(&json) {
        Ok(b) => Box::into_raw(Box::new(b)) as jlong,
        Err(e) => {
            throw(&mut env, e);
            0
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Engine_nativeMatchBundle<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    pattern_ptr: jlong,
    bundle_ptr: jlong,
) -> jstring {
    let pattern = unsafe { &*(pattern_ptr as *const stix_ffi::Pattern) };
    let bundle = unsafe { &*(bundle_ptr as *const stix_ffi::Bundle) };
    // NOTE: match_bundle lives on Engine in stix-ffi; use a throwaway engine (it holds
    // no per-call state relevant to matching).
    let engine = stix_ffi::Engine::new();
    match engine.match_bundle(pattern, bundle) {
        Ok(o) => {
            let json = format!(
                "{{\"matched\":{},\"observations\":{:?}}}",
                o.matched, o.observations
            );
            out_string(&mut env, json)
        }
        Err(e) => {
            throw(&mut env, e);
            std::ptr::null_mut()
        }
    }
}

// --- Pattern ---

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Pattern_nativeAst<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
) -> jstring {
    let pattern = unsafe { &*(ptr as *const stix_ffi::Pattern) };
    out_string(&mut env, pattern.to_json())
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Pattern_nativeFree<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
) {
    if ptr != 0 {
        unsafe { drop(Box::from_raw(ptr as *mut stix_ffi::Pattern)) };
    }
}

// --- Bundle ---

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Bundle_nativeObjectCount<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
) -> jint {
    let bundle = unsafe { &*(ptr as *const stix_ffi::Bundle) };
    bundle.object_count() as jint
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Bundle_nativeObject<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
    index: jint,
) -> jstring {
    let bundle = unsafe { &*(ptr as *const stix_ffi::Bundle) };
    match bundle.object_json(index as usize) {
        Some(json) => out_string(&mut env, json),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_Bundle_nativeFree<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
    ptr: jlong,
) {
    if ptr != 0 {
        unsafe { drop(Box::from_raw(ptr as *mut stix_ffi::Bundle)) };
    }
}
