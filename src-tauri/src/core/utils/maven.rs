pub fn maven_to_path(name: &str, override_classifier: Option<&str>) -> String {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 { return name.to_string(); }

    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    
    let mut version = parts[2];
    let mut ext = "jar";

    if version.contains('@') {
        let v_parts: Vec<&str> = version.split('@').collect();
        version = v_parts[0];
        ext = v_parts[1];
    }

    let mut classifier = if let Some(c) = override_classifier {
        format!("-{}", c)
    } else {
        "".to_string()
    };

    if parts.len() > 3 {
        let mut c_str = parts[3];
        if c_str.contains('@') {
            let c_parts: Vec<&str> = c_str.split('@').collect();
            c_str = c_parts[0];
            ext = c_parts[1];
        }
        if override_classifier.is_none() {
            classifier = format!("-{}", c_str);
        }
    }

    format!("{}/{}/{}/{}-{}{}.{}", group, artifact, version, artifact, version, classifier, ext)
}