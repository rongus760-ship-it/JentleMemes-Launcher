use crate::core::utils::xml_meta;

pub fn extract_maven_versions_chunked(xml: &str) -> Vec<String> {
    xml_meta::parse_maven_metadata(xml).versions
}
