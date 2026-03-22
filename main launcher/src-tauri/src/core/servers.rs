//! Парсинг servers.dat (NBT) из Minecraft
use quartz_nbt::io::Flavor;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerFromDat {
    pub name: String,
    pub ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// Парсит servers.dat (uncompressed NBT). Возвращает список серверов.
pub fn parse_servers_dat(path: &Path) -> Vec<ServerFromDat> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return vec![],
    };
    let mut reader = BufReader::new(file);
    let (compound, _) = match quartz_nbt::io::read_nbt(&mut reader, Flavor::Uncompressed) {
        Ok(x) => x,
        Err(_) => {
            if let Ok(file) = File::open(path) {
                let mut r = BufReader::new(file);
                if let Ok(x) = quartz_nbt::io::read_nbt(&mut r, Flavor::GzCompressed) {
                    x
                } else {
                    return vec![];
                }
            } else {
                return vec![];
            }
        }
    };
    let servers_list = match compound.get::<_, &quartz_nbt::NbtList>("servers") {
        Ok(l) => l,
        Err(_) => return vec![],
    };
    let mut out = Vec::new();
    for tag in servers_list.as_ref() {
        let comp = match tag {
            quartz_nbt::NbtTag::Compound(c) => c,
            _ => continue,
        };
        let ip: String = comp.get::<_, &str>("ip").map(|s| s.to_string()).unwrap_or_default();
        let name: String = comp.get::<_, &str>("name").map(|s| s.to_string()).unwrap_or_else(|_| ip.clone());
        if ip.is_empty() {
            continue;
        }
        let icon: Option<String> = comp.get::<_, &str>("icon").ok()
            .and_then(|s| if s.is_empty() { None } else { Some(format!("data:image/png;base64,{}", s)) });
        out.push(ServerFromDat { name, ip, icon });
    }
    out
}

/// Собирает серверы из servers.dat: ~/.minecraft и из каждой директории инстанса
pub fn collect_servers_from_dat() -> Vec<ServerFromDat> {
    use std::collections::HashMap;
    let mut by_ip: HashMap<String, ServerFromDat> = HashMap::new();
    let data_dir = crate::config::get_data_dir();

    // ~/.minecraft/servers.dat
    if let Some(home) = dirs::home_dir() {
        let mc = home.join(".minecraft").join("servers.dat");
        for srv in parse_servers_dat(&mc) {
            by_ip.entry(srv.ip.clone()).or_insert(srv);
        }
    }

    // Каждый инстанс: instances/<id>/servers.dat
    let instances_dir = data_dir.join("instances");
    if let Ok(entries) = std::fs::read_dir(instances_dir) {
        for entry in entries.flatten() {
            let dat_path = entry.path().join("servers.dat");
            for srv in parse_servers_dat(&dat_path) {
                by_ip.entry(srv.ip.clone()).or_insert(srv);
            }
        }
    }

    by_ip.into_values().collect()
}

/// Collects servers from a specific instance's servers.dat only
pub fn collect_servers_from_instance_dat(instance_id: &str) -> Vec<ServerFromDat> {
    let path = crate::config::get_data_dir()
        .join("instances")
        .join(instance_id)
        .join("servers.dat");
    parse_servers_dat(&path)
}
