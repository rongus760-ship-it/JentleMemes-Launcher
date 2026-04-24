use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::mpsc::{self, Sender};
use std::sync::OnceLock;
use std::thread;

pub const DISCORD_APPLICATION_ID: &str = "1485313092694048849";

const MAX_DISCORD_FIELD: usize = 120;

fn truncate(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() <= MAX_DISCORD_FIELD {
        return t.to_string();
    }
    let mut out = String::new();
    for ch in t.chars() {
        if out.chars().count() >= MAX_DISCORD_FIELD.saturating_sub(1) {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

enum Command {
    Set {
        details: String,
        state: String,
        start_unix_ms: i64,
    },
    Clear,
}

static SENDER: OnceLock<Sender<Command>> = OnceLock::new();

fn log_discord_err(context: &str, err: &discord_rich_presence::error::Error) {
    eprintln!(
        "[discord] {}: {:?} — проверьте: Discord **запущен** (не веб-версия), лаунчер не в изоляции без доступа к сокету discord-ipc (Flatpak Discord + нативный лаунчер часто **не видят** друг друга; попробуйте нативный Discord или Discord из того же Flatpak).",
        context, err
    );
}

fn ensure_connected(client: &mut Option<DiscordIpcClient>) -> bool {
    if DISCORD_APPLICATION_ID.is_empty() {
        return false;
    }
    if client.is_some() {
        return true;
    }
    let mut c = DiscordIpcClient::new(DISCORD_APPLICATION_ID);
    match c.connect() {
        Ok(()) => {
            *client = Some(c);
            true
        }
        Err(e) => {
            log_discord_err("подключение к Discord IPC", &e);
            false
        }
    }
}

fn ensure_worker() -> &'static Sender<Command> {
    SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Command>();
        thread::spawn(move || {
            let mut client: Option<DiscordIpcClient> = None;
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    Command::Set {
                        details,
                        state,
                        start_unix_ms,
                    } => {
                        if DISCORD_APPLICATION_ID.is_empty() {
                            continue;
                        }
                        if !ensure_connected(&mut client) {
                            continue;
                        }
                        if let Some(ref mut c) = client {
                            let act = activity::Activity::new()
                                .name("Minecraft")
                                .activity_type(activity::ActivityType::Playing)
                                .details(details)
                                .state(state)
                                .timestamps(activity::Timestamps::new().start(start_unix_ms));
                            if let Err(e) = c.set_activity(act) {
                                log_discord_err("set_activity", &e);
                                let _ = c.close();
                                client = None;
                                continue;
                            }
                            // Считать ответ Discord, иначе буфер может рассинхронизироваться
                            if let Err(e) = c.recv() {
                                log_discord_err("recv после set_activity", &e);
                                let _ = c.close();
                                client = None;
                            }
                        }
                    }
                    Command::Clear => {
                        if let Some(ref mut c) = client {
                            let _ = c.clear_activity();
                            let _ = c.recv();
                        }
                    }
                }
            }
        });
        tx
    })
}

/// Показать «играет» в Minecraft для сборки.
pub fn set_playing_minecraft(
    instance_name: &str,
    game_version: &str,
    loader: &str,
    loader_version: &str,
    server_ip: Option<&str>,
) {
    if DISCORD_APPLICATION_ID.is_empty() {
        return;
    }
    // В discord-rich-presence для Timestamps указаны **миллисекунды** с Unix epoch
    let start_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let mut state = format!(
        "{} · {}{}",
        game_version,
        loader,
        if loader_version.is_empty() {
            String::new()
        } else {
            format!(" {}", loader_version)
        }
    );
    if let Some(ip) = server_ip {
        if !ip.is_empty() {
            state = format!(
                "{} · {}",
                truncate(&state),
                truncate(&format!("Сервер: {}", ip))
            );
        }
    }

    let _ = ensure_worker().send(Command::Set {
        details: truncate(instance_name),
        state: truncate(&state),
        start_unix_ms,
    });
}

pub fn clear() {
    if DISCORD_APPLICATION_ID.is_empty() {
        return;
    }
    let _ = ensure_worker().send(Command::Clear);
}
