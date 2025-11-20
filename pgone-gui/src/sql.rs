use eframe::egui::text::{LayoutJob, TextFormat};
use sqlx::Row;
use sqlx::postgres::PgRow;

pub fn highlight_sql(text: &str, visuals: &egui::Visuals) -> LayoutJob {
    let mut job = LayoutJob::default();
    let normal = TextFormat {
        color: visuals.text_color(),
        ..Default::default()
    };
    let kw = TextFormat {
        color: egui::Color32::from_rgb(198, 120, 221),
        ..Default::default()
    };
    let string = TextFormat {
        color: egui::Color32::from_rgb(152, 195, 121),
        ..Default::default()
    };
    let number = TextFormat {
        color: egui::Color32::from_rgb(209, 154, 102),
        ..Default::default()
    };
    let mut i = 0;
    let s = text.as_bytes();
    while i < s.len() {
        let c = s[i] as char;
        if c.is_whitespace() {
            job.append(&text[i..i + 1], 0.0, normal.clone());
            i += 1;
            continue;
        }
        if c == '\'' {
            let start = i;
            i += 1;
            while i < s.len() {
                if s[i] as char == '\'' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            job.append(&text[start..i], 0.0, string.clone());
            continue;
        }
        let start = i;
        while i < s.len() {
            let ch = s[i] as char;
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            i += 1;
        }
        let token = &text[start..i];
        let lower = token.to_ascii_lowercase();
        let fmt = if matches!(
            lower.as_str(),
            "select"
                | "from"
                | "where"
                | "insert"
                | "update"
                | "delete"
                | "join"
                | "left"
                | "right"
                | "on"
                | "group"
                | "by"
                | "order"
                | "limit"
                | "offset"
                | "and"
                | "or"
                | "not"
                | "as"
        ) {
            kw.clone()
        } else if token.chars().all(|ch| ch.is_ascii_digit()) {
            number.clone()
        } else {
            normal.clone()
        };
        job.append(token, 0.0, fmt);
    }
    job
}

pub fn format_cell(row: &PgRow, idx: usize) -> String {
    if let Ok(v) = row.try_get::<String, _>(idx) {
        return v;
    }
    if let Ok(v) = row.try_get::<i64, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<f64, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<bool, _>(idx) {
        return v.to_string();
    }
    if let Ok(v) = row.try_get::<Vec<u8>, _>(idx) {
        return format!("\\x{}", hex::encode(v));
    }
    "<unfmt>".to_string()
}
