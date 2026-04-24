//! Плоские строки MOTD из поля description (строка / chat JSON / массив).

use serde_json::Value;

fn push_line(out: &mut Vec<String>, line: &str) {
    let t = line.trim_end();
    if !t.is_empty() {
        out.push(strip_legacy_codes_for_clean(t));
    }
}

/// Убирает §коды для «чистой» подписи (превью одной строкой).
fn strip_legacy_codes_for_clean(s: &str) -> String {
    let mut r = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '§' {
            it.next();
            continue;
        }
        r.push(c);
    }
    r
}

fn append_chat_text(v: &Value, out: &mut Vec<String>, line_buf: &mut String) {
    match v {
        Value::String(s) => {
            let parts: Vec<&str> = s.split('\n').collect();
            for (i, part) in parts.iter().enumerate() {
                line_buf.push_str(part);
                if i + 1 < parts.len() {
                    push_line(out, line_buf);
                    line_buf.clear();
                }
            }
        }
        Value::Object(o) => {
            if let Some(t) = o.get("text").and_then(|x| x.as_str()) {
                line_buf.push_str(t);
            }
            if let Some(extra) = o.get("extra").and_then(|x| x.as_array()) {
                for e in extra {
                    append_chat_text(e, out, line_buf);
                }
            }
        }
        Value::Array(a) => {
            for e in a {
                append_chat_text(e, out, line_buf);
            }
        }
        _ => {}
    }
}

pub fn description_to_clean_lines(description: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut line_buf = String::new();
    match description {
        Value::String(s) => {
            for part in s.split('\n') {
                push_line(&mut out, part);
            }
        }
        Value::Object(_) | Value::Array(_) => {
            append_chat_text(description, &mut out, &mut line_buf);
            if !line_buf.is_empty() {
                push_line(&mut out, &line_buf);
            }
        }
        _ => {}
    }
    if out.is_empty() {
        out.push("Сервер работает".into());
    }
    out
}
