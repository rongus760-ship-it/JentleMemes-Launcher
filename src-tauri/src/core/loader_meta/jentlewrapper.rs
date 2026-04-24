//! Вендорная копия ForgeWrapper (upstream 1.6.0), встроенная в бинарник лаунчера.

/// Maven-координаты: версия — «имя релиза» по запросу продукта.
pub const JENTLE_WRAPPER_COORD: &str = "wtf.jentlememes:wrapper:JentleWrapper-alpha.1-prerelease.27n3_hotfix-17246502_signature--223.no-cfg";

static JAR_BYTES: &[u8] = include_bytes!("../../../resources/jentlewrapper-vendor.jar");

pub const JENTLE_WRAPPER_SHA1: &str = "035a51fe6439792a61507630d89382f621da0f1f";
pub const JENTLE_WRAPPER_SIZE: u64 = 28679;

pub fn embedded_jar_bytes() -> &'static [u8] {
    JAR_BYTES
}
