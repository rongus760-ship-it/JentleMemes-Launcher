use std::collections::VecDeque;
use std::io::Write;
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::ChildStdout;
use tokio::process::ChildStderr;

const RING_CAPACITY: usize = 4096;
const BATCH_INTERVAL_MS: u64 = 80;

pub fn spawn_pipe_readers(
    stdout: ChildStdout,
    stderr: ChildStderr,
    app: tauri::AppHandle,
    instance_id: String,
    console_file: Option<Arc<Mutex<std::fs::File>>>,
) {
    let ring: Arc<Mutex<VecDeque<String>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(RING_CAPACITY)));

    let ring_out = ring.clone();
    let cf_out = console_file.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim_end().to_string();
                    if let Some(ref f) = cf_out {
                        if let Ok(mut g) = f.lock() {
                            let _ = writeln!(g, "{}", trimmed);
                        }
                    }
                    let mut ring = ring_out.lock().unwrap();
                    if ring.len() >= RING_CAPACITY {
                        ring.pop_front();
                    }
                    ring.push_back(trimmed);
                }
                Err(_) => break,
            }
        }
    });

    let ring_err = ring.clone();
    let cf_err = console_file;
    tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let trimmed = line.trim_end().to_string();
                    if let Some(ref f) = cf_err {
                        if let Ok(mut g) = f.lock() {
                            let _ = writeln!(g, "{}", trimmed);
                        }
                    }
                    let mut ring = ring_err.lock().unwrap();
                    if ring.len() >= RING_CAPACITY {
                        ring.pop_front();
                    }
                    ring.push_back(trimmed);
                }
                Err(_) => break,
            }
        }
    });

    let event_name = format!("log_{}", instance_id);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(BATCH_INTERVAL_MS)).await;
            let batch: Vec<String> = {
                let mut ring = ring.lock().unwrap();
                let drained: Vec<String> = ring.drain(..).collect();
                drained
            };
            if batch.is_empty() {
                continue;
            }
            let joined = batch.join("\n");
            let _ = app.emit(&event_name, joined);
        }
    });
}
