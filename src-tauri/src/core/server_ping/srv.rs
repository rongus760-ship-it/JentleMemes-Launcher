//! `_minecraft._tcp` (SRV) по RFC 2782.

use hickory_resolver::proto::rr::rdata::SRV;
use hickory_resolver::TokioAsyncResolver;
use once_cell::sync::Lazy;
use rand::Rng;
use tokio::time::{timeout, Duration};

static RESOLVER: Lazy<TokioAsyncResolver> = Lazy::new(|| {
    TokioAsyncResolver::tokio_from_system_conf().unwrap_or_else(|_| {
        TokioAsyncResolver::tokio(
            hickory_resolver::config::ResolverConfig::google(),
            hickory_resolver::config::ResolverOpts::default(),
        )
    })
});

/// Если systemd-resolved / локальный DNS не отдаёт SRV, повторяем запрос через публичные сервера.
static RESOLVER_GOOGLE: Lazy<TokioAsyncResolver> = Lazy::new(|| {
    TokioAsyncResolver::tokio(
        hickory_resolver::config::ResolverConfig::google(),
        hickory_resolver::config::ResolverOpts::default(),
    )
});

static RESOLVER_CLOUDFLARE: Lazy<TokioAsyncResolver> = Lazy::new(|| {
    TokioAsyncResolver::tokio(
        hickory_resolver::config::ResolverConfig::cloudflare(),
        hickory_resolver::config::ResolverOpts::default(),
    )
});

/// Без верхней границы `srv_lookup` может зависнуть на сломанном systemd-resolved / NSS → UI вечное «Загрузка…».
const SRV_LOOKUP_TIMEOUT: Duration = Duration::from_secs(4);

fn srv_target_host(rec: &SRV) -> String {
    rec.target().to_utf8().trim_end_matches('.').to_string()
}

fn pick_srv_record(records: Vec<SRV>) -> Option<(String, u16)> {
    if records.is_empty() {
        return None;
    }
    let mut records = records;
    records.sort_by_key(|r| r.priority());
    let best_pri = records[0].priority();
    let group: Vec<SRV> = records
        .into_iter()
        .filter(|r| r.priority() == best_pri)
        .collect();
    if group.is_empty() {
        return None;
    }

    let non_zero: Vec<&SRV> = group.iter().filter(|r| r.weight() > 0).collect();
    if non_zero.is_empty() {
        let r = group.first()?;
        return Some((srv_target_host(r), r.port()));
    }

    let total: u32 = non_zero.iter().map(|r| r.weight() as u32).sum();
    let mut roll = rand::thread_rng().gen_range(0..total);
    for r in non_zero {
        let w = r.weight() as u32;
        if roll < w {
            return Some((srv_target_host(r), r.port()));
        }
        roll -= w;
    }
    let r = group.last()?;
    Some((srv_target_host(r), r.port()))
}

async fn srv_lookup_on(resolver: &TokioAsyncResolver, host: &str) -> Option<(String, u16)> {
    let qname = format!("_minecraft._tcp.{host}.");
    let lookup = match timeout(SRV_LOOKUP_TIMEOUT, resolver.srv_lookup(qname.as_str())).await {
        Ok(Ok(l)) => l,
        _ => return None,
    };
    pick_srv_record(lookup.into_iter().collect())
}

/// Запись `_minecraft._tcp.<hostname>.` → хост и порт для TCP.
pub async fn try_minecraft_srv(hostname: &str) -> Option<(String, u16)> {
    let host = hostname.trim().trim_end_matches('.');
    if host.is_empty() {
        return None;
    }
    if host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }

    if let Some(hit) = srv_lookup_on(&RESOLVER, host).await {
        return Some(hit);
    }
    if let Some(hit) = srv_lookup_on(&RESOLVER_GOOGLE, host).await {
        return Some(hit);
    }
    srv_lookup_on(&RESOLVER_CLOUDFLARE, host).await
}
