use crate::core::types::VersionInfo;

pub struct AssetResult {
    pub asset_index: String,
    pub assets_dir: String,
}

pub fn resolve(
    all_v_infos: &[VersionInfo],
    data_dir: &std::path::Path,
) -> AssetResult {
    let main_v = all_v_infos.last().unwrap();
    let vanilla_v = all_v_infos.first().unwrap();

    let asset_index = main_v
        .assets
        .clone()
        .or_else(|| vanilla_v.assets.clone())
        .unwrap_or_else(|| "legacy".into());

    let assets_dir = data_dir
        .join("assets")
        .to_string_lossy()
        .replace('\\', "/");

    AssetResult {
        asset_index,
        assets_dir,
    }
}
