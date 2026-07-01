//! JNI bindings for the stix-rust toolkit (raw layer).
use jni::objects::JClass;
use jni::sys::jlong;
use jni::JNIEnv;

#[no_mangle]
pub extern "system" fn Java_io_github_benjaminsmall_stix_NativeLoader_nativeHealthcheck<'l>(
    _env: JNIEnv<'l>,
    _class: JClass<'l>,
) -> jlong {
    1
}
