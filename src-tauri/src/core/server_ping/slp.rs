//! Java Edition Server List Ping (TCP), без сторонних HTTP API.
//! Поддержка SRV `_minecraft._tcp` при отсутствии явного порта в строке.

use serde_json::Value;
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

use crate::error::{Error, Result};

use super::address::{handshake_host_string, is_ip_literal, parse_server_address};
use super::srv::try_minecraft_srv;
use super::varint::{read_string_slice, read_varint_slice, write_string, write_varint};

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_PACKET: usize = 4 * 1024 * 1024;

pub use super::address::parse_server_address_legacy;

/// Куда подключаться и что передать в handshake (виртуальный хост Bungee / Velocity).
#[derive(Debug, Clone)]
pub struct ResolvedPingTarget {
    /// Имя из адреса пользователя (без порта), для ответа UI.
    pub logical_host: String,
    pub connect_host: String,
    pub connect_port: u16,
    pub handshake_host: String,
    pub handshake_port: u16,
    pub srv_used: bool,
    /// Цель SRV (если была), иначе `None`.
    pub srv_target: Option<String>,
}

pub async fn resolve_ping_target(addr: &str) -> Result<ResolvedPingTarget> {
    let (logical_host, port, port_explicit) = parse_server_address(addr);
    if logical_host.is_empty() {
        return Err(Error::Custom("Пустой адрес сервера".into()));
    }

    let handshake_host = handshake_host_string(&logical_host);

    if port_explicit || is_ip_literal(&logical_host) {
        return Ok(ResolvedPingTarget {
            logical_host: logical_host.clone(),
            connect_host: logical_host,
            connect_port: port,
            handshake_host,
            handshake_port: port,
            srv_used: false,
            srv_target: None,
        });
    }

    if let Some((target, srv_port)) = try_minecraft_srv(&logical_host).await {
        return Ok(ResolvedPingTarget {
            logical_host: logical_host.clone(),
            connect_host: target.clone(),
            connect_port: srv_port,
            handshake_host,
            handshake_port: srv_port,
            srv_used: true,
            srv_target: Some(target),
        });
    }

    Ok(ResolvedPingTarget {
        logical_host: logical_host.clone(),
        connect_host: logical_host.clone(),
        connect_port: 25565,
        handshake_host,
        handshake_port: 25565,
        srv_used: false,
        srv_target: None,
    })
}

async fn read_varint_stream<R: AsyncRead + Unpin>(r: &mut R) -> Result<i32> {
    let mut num = 0i32;
    let mut shift = 0;
    for _ in 0..5 {
        let b = r
            .read_u8()
            .await
            .map_err(|e| Error::Custom(e.to_string()))?;
        num |= ((b & 0x7F) as i32) << shift;
        if b & 0x80 == 0 {
            return Ok(num);
        }
        shift += 7;
    }
    Err(Error::Custom("VarInt из потока: переполнение".into()))
}

fn build_handshake(handshake_host: &str, handshake_port: u16) -> Result<Vec<u8>> {
    let mut pkt = Vec::new();
    write_varint(&mut pkt, 0x00);
    // 763 ≈ Minecraft 1.20.1 — многие прокси/ядра отклоняют -1; для state=status это безопасная «универсальная» версия.
    write_varint(&mut pkt, 763);
    write_string(&mut pkt, handshake_host)?;
    pkt.extend_from_slice(&handshake_port.to_be_bytes());
    write_varint(&mut pkt, 1);

    let mut frame = Vec::new();
    write_varint(&mut frame, pkt.len() as i32);
    frame.extend_from_slice(&pkt);
    Ok(frame)
}

fn build_status_request() -> Vec<u8> {
    let mut pkt = Vec::new();
    write_varint(&mut pkt, 0x00);
    let mut frame = Vec::new();
    write_varint(&mut frame, pkt.len() as i32);
    frame.extend_from_slice(&pkt);
    frame
}

async fn tcp_connect_host_port(host: &str, port: u16) -> Result<TcpStream> {
    let h = host.trim();
    let inner = async {
        if let Ok(ip) = h.parse::<IpAddr>() {
            return TcpStream::connect(SocketAddr::new(ip, port)).await;
        }
        let addrs = tokio::net::lookup_host((h, port)).await?;
        let mut last_err: Option<std::io::Error> = None;
        for addr in addrs {
            match TcpStream::connect(addr).await {
                Ok(s) => return Ok(s),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "нет доступных IP-адресов для подключения",
            )
        }))
    };
    match timeout(READ_TIMEOUT, inner).await {
        Err(_) => Err(Error::Custom("Таймаут подключения к серверу".into())),
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(e)) => Err(Error::Custom(format!(
            "Не удалось подключиться к «{host}:{port}» (TCP/DNS): {e}"
        ))),
    }
}

async fn fetch_status_json_once(addr: &str) -> Result<(Value, ResolvedPingTarget)> {
    let target = resolve_ping_target(addr).await?;
    let mut stream = tcp_connect_host_port(&target.connect_host, target.connect_port).await?;
    let _ = stream.set_nodelay(true);

    let hs = build_handshake(&target.handshake_host, target.handshake_port)?;
    stream
        .write_all(&hs)
        .await
        .map_err(|e| Error::Custom(e.to_string()))?;
    stream
        .write_all(&build_status_request())
        .await
        .map_err(|e| Error::Custom(e.to_string()))?;

    let pkt_len = timeout(READ_TIMEOUT, read_varint_stream(&mut stream))
        .await
        .map_err(|_| Error::Custom("Таймаут ответа сервера".into()))??;
    if pkt_len < 0 || pkt_len as usize > MAX_PACKET {
        return Err(Error::Custom("Некорректная длина пакета статуса".into()));
    }
    let len = pkt_len as usize;
    let mut buf = vec![0u8; len];
    timeout(READ_TIMEOUT, stream.read_exact(&mut buf))
        .await
        .map_err(|_| Error::Custom("Таймаут чтения статуса".into()))?
        .map_err(|e| Error::Custom(e.to_string()))?;

    let mut cur = &buf[..];
    let _pid = read_varint_slice(&mut cur)?;
    let json_str = read_string_slice(&mut cur)?;
    let value =
        serde_json::from_str(&json_str).map_err(|e| Error::Custom(format!("JSON статуса: {e}")))?;
    Ok((value, target))
}

/// Сырой JSON статуса + цель подключения (SRV / явный порт). Две попытки — обход редких сбоев TCP.
pub async fn fetch_status_json(addr: &str) -> Result<(Value, ResolvedPingTarget)> {
    let mut last = String::from("нет попыток");
    for attempt in 0..2 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        match fetch_status_json_once(addr).await {
            Ok(v) => return Ok(v),
            Err(e) => last = e.to_string(),
        }
    }
    Err(Error::Custom(format!("Статус сервера (2 попытки): {last}")))
}
