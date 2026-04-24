pub struct ArgResult {
    pub args: Vec<String>,
}

pub fn resolve(
    _java_path: &str,
    ram_mb: u32,
    jvm_args_extra: &str,
    classpath: &[String],
    main_class: &str,
) -> ArgResult {
    let mut args: Vec<String> = Vec::new();

    args.push("-XX:+UnlockExperimentalVMOptions".into());
    args.push(format!("-Xmx{}M", ram_mb));

    for part in jvm_args_extra.split_whitespace() {
        if !part.is_empty() {
            args.push(part.to_string());
        }
    }

    #[cfg(not(target_os = "windows"))]
    let sep = ":";
    #[cfg(target_os = "windows")]
    let sep = ";";

    args.push("-cp".into());
    args.push(classpath.join(sep));
    args.push(main_class.to_string());

    ArgResult { args }
}
