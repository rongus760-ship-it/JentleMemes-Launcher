//! In-memory cache for loader meta HTTP bodies (TTL 2h, сброс при перезапуске процесса).

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;

const TTL: Duration = Duration::from_secs(2 * 3600);

static BODY_CACHE: Lazy<Mutex<HashMap<String, (Instant, String)>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn get_cached(key: &str) -> Option<String> {
    let g = BODY_CACHE.lock().ok()?;
    let (t, body) = g.get(key)?;
    if t.elapsed() < TTL {
        Some(body.clone())
    } else {
        None
    }
}

pub fn put_cached(key: &str, body: String) {
    if let Ok(mut g) = BODY_CACHE.lock() {
        g.insert(key.to_string(), (Instant::now(), body));
    }
}
