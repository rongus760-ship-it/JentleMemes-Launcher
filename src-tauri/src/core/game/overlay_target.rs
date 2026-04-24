//! Прямоугольник окна Minecraft / запасной монитор для оверлея (Windows, Linux X11, macOS).

use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};

#[derive(Debug, Clone, Serialize)]
pub struct OverlayTargetRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    /// "game" | "monitor" | "fallback"
    pub source: String,
}

const MIN_W: i32 = 320;
const MIN_H: i32 = 240;

fn primary_monitor_rect() -> Option<OverlayTargetRect> {
    let displays = display_info::DisplayInfo::all().ok()?;
    let d = displays.into_iter().find(|x| x.is_primary)?;
    Some(OverlayTargetRect {
        x: d.x,
        y: d.y,
        width: d.width,
        height: d.height,
        source: "monitor".into(),
    })
}

/// Глобальный переиспользуемый `System` для sysinfo. `refresh_processes` на уже
/// проинициализированном `System` в 3–5 раз быстрее, чем каждый раз делать
/// `System::new()`. Это важно, т.к. оверлей пулит статистику каждые ~800 мс.
static SYSINFO: Lazy<Mutex<System>> = Lazy::new(|| Mutex::new(System::new()));

/// Короткоживущий кеш результата overlay_target / stats, чтобы при частом пулинге
/// (особенно когда оверлей и бэк на одной машине) не насиловать X11 и sysinfo.
struct OverlayCache<T: Clone> {
    value: Option<T>,
    at: Option<Instant>,
}

impl<T: Clone> OverlayCache<T> {
    const fn new() -> Self {
        Self {
            value: None,
            at: None,
        }
    }
    fn get(&self, ttl: Duration) -> Option<T> {
        let at = self.at?;
        if at.elapsed() <= ttl {
            self.value.clone()
        } else {
            None
        }
    }
    fn set(&mut self, v: T) {
        self.value = Some(v);
        self.at = Some(Instant::now());
    }
}

static RECT_CACHE: Lazy<Mutex<OverlayCache<OverlayTargetRect>>> =
    Lazy::new(|| Mutex::new(OverlayCache::new()));
static STATS_CACHE: Lazy<Mutex<OverlayCache<GameOverlayStats>>> =
    Lazy::new(|| Mutex::new(OverlayCache::new()));

const RECT_TTL: Duration = Duration::from_millis(350);
const STATS_TTL: Duration = Duration::from_millis(900);

fn expand_pid_tree(roots: &[u32]) -> Vec<u32> {
    if roots.is_empty() {
        return vec![];
    }
    let mut sys = SYSINFO.lock().unwrap();
    sys.refresh_processes();
    let root_set: HashSet<u32> = roots.iter().copied().collect();
    let mut set: HashSet<u32> = root_set.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for (pid, proc_) in sys.processes() {
            let p = pid.as_u32();
            if set.contains(&p) {
                continue;
            }
            if let Some(par) = proc_.parent() {
                if set.contains(&par.as_u32()) && set.insert(p) {
                    changed = true;
                }
            }
        }
    }
    set.into_iter().collect()
}

#[cfg(target_os = "windows")]
fn rect_for_pids_win(pids: &[u32]) -> Option<OverlayTargetRect> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
    };

    if pids.is_empty() {
        return None;
    }
    let set: HashSet<u32> = pids.iter().copied().collect();

    struct Ctx {
        pids: HashSet<u32>,
        best_area: i64,
        best: Option<RECT>,
    }

    unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut Ctx);
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return TRUE;
        }
        let mut wpid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut wpid));
        if !ctx.pids.contains(&wpid) {
            return TRUE;
        }
        let mut r = RECT::default();
        if GetWindowRect(hwnd, &mut r).is_err() {
            return TRUE;
        }
        let w = (r.right - r.left) as i64;
        let h = (r.bottom - r.top) as i64;
        if w < MIN_W as i64 || h < MIN_H as i64 {
            return TRUE;
        }
        let area = w * h;
        if area > ctx.best_area {
            ctx.best_area = area;
            ctx.best = Some(r);
        }
        TRUE
    }

    let mut ctx = Ctx {
        pids: set,
        best_area: 0,
        best: None,
    };
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut ctx as *mut Ctx as isize));
    }
    let r = ctx.best?;
    let w = (r.right - r.left).max(0) as u32;
    let h = (r.bottom - r.top).max(0) as u32;
    if w < MIN_W as u32 || h < MIN_H as u32 {
        return None;
    }
    Some(OverlayTargetRect {
        x: r.left,
        y: r.top,
        width: w,
        height: h,
        source: "game".into(),
    })
}

#[cfg(target_os = "linux")]
fn rect_for_pids_x11(pids: &[u32]) -> Option<OverlayTargetRect> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, Window};

    if pids.is_empty() || std::env::var("DISPLAY").is_err() {
        return None;
    }
    let set: HashSet<u32> = pids.iter().copied().collect();
    let (conn, screen_idx) = x11rb::connect(None).ok()?;
    let root = conn.setup().roots[screen_idx].root;
    let atom_pid = conn
        .intern_atom(false, b"_NET_WM_PID")
        .ok()?
        .reply()
        .ok()?
        .atom;

    let mut best_area: i64 = 0;
    let mut best: Option<(i32, i32, u32, u32)> = None;

    fn walk<C: Connection + ?Sized>(
        conn: &C,
        atom_pid: u32,
        root: Window,
        wid: Window,
        pids: &HashSet<u32>,
        best_area: &mut i64,
        best: &mut Option<(i32, i32, u32, u32)>,
    ) {
        let Ok(cookie) = conn.query_tree(wid) else {
            return;
        };
        let Ok(tree) = cookie.reply() else {
            return;
        };
        for &child in &tree.children {
            let rep = match conn.get_property(false, child, atom_pid, AtomEnum::CARDINAL, 0, 1) {
                Ok(c) => c.reply().ok(),
                Err(_) => None,
            };
            if let Some(rep) = rep {
                if rep.format == 32 && rep.value.len() >= 4 {
                    let pid = u32::from_ne_bytes(rep.value[0..4].try_into().unwrap());
                    if pids.contains(&pid) {
                        let g = match conn.get_geometry(child) {
                            Ok(c) => c.reply().ok(),
                            Err(_) => None,
                        };
                        let tr = match conn.translate_coordinates(child, root, 0, 0) {
                            Ok(c) => c.reply().ok(),
                            Err(_) => None,
                        };
                        if let (Some(g), Some(tr)) = (g, tr) {
                            let x = tr.dst_x as i32;
                            let y = tr.dst_y as i32;
                            let w = g.width as i32;
                            let h = g.height as i32;
                            if w >= MIN_W && h >= MIN_H {
                                let area = (w as i64) * (h as i64);
                                if area > *best_area {
                                    *best_area = area;
                                    *best = Some((x, y, w as u32, h as u32));
                                }
                            }
                        }
                    }
                }
            }
            walk(conn, atom_pid, root, child, pids, best_area, best);
        }
    }

    walk(&conn, atom_pid, root, root, &set, &mut best_area, &mut best);
    let (x, y, w, h) = best?;
    Some(OverlayTargetRect {
        x,
        y,
        width: w,
        height: h,
        source: "game".into(),
    })
}

/// На macOS пока без привязки к HWND игры (sandbox / API): оверлей на весь основной монитор.
#[cfg(target_os = "macos")]
fn rect_for_pids_macos(_pids: &[u32]) -> Option<OverlayTargetRect> {
    None
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn rect_for_pids_stub(_pids: &[u32]) -> Option<OverlayTargetRect> {
    None
}

pub fn resolve_overlay_rect(roots: &[u32]) -> OverlayTargetRect {
    if let Some(cached) = RECT_CACHE.lock().unwrap().get(RECT_TTL) {
        return cached;
    }
    let expanded = expand_pid_tree(roots);

    #[cfg(target_os = "windows")]
    let game = rect_for_pids_win(&expanded);

    #[cfg(target_os = "linux")]
    let game = rect_for_pids_x11(&expanded);

    #[cfg(target_os = "macos")]
    let game = rect_for_pids_macos(&expanded);

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    let game = rect_for_pids_stub(&expanded);

    let out = game
        .or_else(primary_monitor_rect)
        .unwrap_or(OverlayTargetRect {
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
            source: "fallback".into(),
        });
    RECT_CACHE.lock().unwrap().set(out.clone());
    out
}

#[derive(Debug, Clone, Serialize)]
pub struct GameOverlayStats {
    pub sessions: usize,
    pub instance_ids: Vec<String>,
    pub memory_used_mb: f64,
    pub cpu_percent_total: f32,
    pub pids: Vec<u32>,
}

pub fn overlay_game_stats() -> GameOverlayStats {
    if let Some(cached) = STATS_CACHE.lock().unwrap().get(STATS_TTL) {
        return cached;
    }

    let map = super::launch::running_sessions_pid_map();
    let sessions = map.len();
    let instance_ids: Vec<String> = map.iter().map(|(id, _)| id.clone()).collect();
    let roots: Vec<u32> = map.iter().map(|(_, p)| *p).collect();
    let expanded = expand_pid_tree(&roots);

    let (mem, cpu) = {
        let mut sys = SYSINFO.lock().unwrap();
        sys.refresh_cpu();
        sys.refresh_processes();
        let mut mem: u64 = 0;
        let mut cpu: f32 = 0.0;
        for pid_u in &expanded {
            let pid = Pid::from_u32(*pid_u);
            if let Some(proc_) = sys.process(pid) {
                mem += proc_.memory();
                cpu += proc_.cpu_usage();
            }
        }
        (mem, cpu)
    };

    let out = GameOverlayStats {
        sessions,
        instance_ids,
        memory_used_mb: mem as f64 / (1024.0 * 1024.0),
        cpu_percent_total: cpu,
        pids: expanded,
    };
    STATS_CACHE.lock().unwrap().set(out.clone());
    out
}
