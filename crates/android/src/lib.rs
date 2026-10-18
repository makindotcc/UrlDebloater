pub use urldebloater_urlwasher::{ffi::*, RedirectWashPolicyDiscriminants, TwitterPolicy};

#[cfg(target_os = "android")]
#[jni::jni_mangle("cc.makin.urldebloater.android.NativeBridge")]
pub fn init_tls<'caller>(
    mut env: jni::EnvUnowned<'caller>,
    _bridge: jni::objects::JClass<'caller>,
    context: jni::objects::JObject<'caller>,
) {
    use jni::errors::ThrowRuntimeExAndDefault;

    env.with_env(|env| rustls_platform_verifier::android::init_with_env(env, context))
        .resolve::<ThrowRuntimeExAndDefault>();
}
