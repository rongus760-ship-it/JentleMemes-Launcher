//! Парсинг адреса сервера и подготовка полей для handshake / SRV.

use std::net::IpAddr;

/// `host`, `port`, был ли порт указан явно в строке (если нет — можно пробовать SRV).
pub fn parse_server_address(input: &str) -> (String, u16, bool) {
    let input = input.trim();
    if input.is_empty() {
        return (String::new(), 25565, false);
    }
    if let Some(rest) = input.strip_prefix('[') {
        if let Some(idx) = rest.find(']') {
            let host = rest[..idx].to_string();
            let after = &rest[idx + 1..];
            if let Some(p) = after.strip_prefix(':') {
                if let Ok(port) = p.parse::<u16>() {
                    return (host, port, true);
                }
            }
            return (host, 25565, false);
        }
    }
    if let Some((h, p)) = input.rsplit_once(':') {
        if p.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(port) = p.parse::<u16>() {
                if h.contains(':') {
                    return (input.to_string(), 25565, false);
                }
                return (h.to_string(), port, true);
            }
        }
    }
    (input.to_string(), 25565, false)
}

/// Старый API: (host, port) без флага явного порта.
pub fn parse_server_address_legacy(input: &str) -> (String, u16) {
    let (h, p, _) = parse_server_address(input);
    (h, p)
}

pub fn is_ip_literal(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok()
}

/// Строка host для handshake (IPv6 в квадратных скобках).
pub fn handshake_host_string(logical_host: &str) -> String {
    match logical_host.parse::<IpAddr>() {
        Ok(IpAddr::V6(_)) => format!("[{logical_host}]"),
        Ok(IpAddr::V4(_)) => logical_host.to_string(),
        Err(_) => logical_host.to_string(),
    }
}
