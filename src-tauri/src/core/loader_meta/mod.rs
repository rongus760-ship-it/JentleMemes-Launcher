//! Метаданные лоадеров без Prism: официальные Fabric/Quilt/Mojang, Forge (promotions + Maven XML), NeoForge (Maven XML).
//! Кэш ответов в памяти на 2 часа; параллельные загрузки ограничены семафором по числу ядер.

mod cache;
mod concurrency;
mod fabric;
mod forge;
mod http;
mod installer_patch;
pub(crate) mod jentlewrapper;
pub mod lists;
pub(crate) mod liteloader;
pub(crate) mod modloader_alpha;
mod modloader_jar_merge;
mod neoforge;
mod quilt;
mod vanilla_mc;
mod version_sort;
mod xml_versions;

pub use installer_patch::{
    forge_profile_patch_from_installer, neoforge_profile_patch_from_installer,
};
pub use jentlewrapper::embedded_jar_bytes;
pub use lists::{loader_game_versions, loader_versions_for_game};
pub use vanilla_mc::manifest_v2;
