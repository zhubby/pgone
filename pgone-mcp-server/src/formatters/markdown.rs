use crate::core::models::*;

pub fn render_overview(db: &DatabaseSchema) -> String {
    let mut s = String::new();
    s.push_str(&format!("数据库：{}\n\n", db.database));
    for sch in &db.schemas {
        s.push_str(&format!("模式 `{}`：\n", sch.name));
        for t in &sch.tables {
            s.push_str(&render_table_line(t));
        }
        for v in &sch.views {
            s.push_str(&format!("- 视图 `{}`\n", v.name));
        }
        s.push('\n');
    }
    s
}

fn render_table_line(t: &TableDetail) -> String {
    let mut s = String::new();
    if let Some(c) = &t.comment {
        s.push_str(&format!("- 表 `{}`（{}）\n", t.name, c));
    } else {
        s.push_str(&format!("- 表 `{}`\n", t.name));
    }
    for col in &t.columns {
        let null = if col.nullable { "可空" } else { "非空" };
        let def = col.default.as_deref().unwrap_or("");
        let comment = col.comment.as_deref().unwrap_or("");
        s.push_str(&format!(
            "  - `{}` {} {} {} {}\n",
            col.name,
            col.data_type,
            null,
            if def.is_empty() { "" } else { "默认" },
            comment
        ));
    }
    if let Some(pk) = &t.primary_key {
        s.push_str(&format!("  - 主键：({})\n", pk.columns.join(", ")))
    }
    for fk in &t.foreign_keys {
        s.push_str(&format!(
            "  - 外键：({}) → {}({})\n",
            fk.columns.join(", "),
            fk.ref_table,
            fk.ref_columns.join(", ")
        ))
    }
    for idx in &t.indexes {
        let inc = if idx.include.is_empty() {
            String::new()
        } else {
            format!(" INCLUDE ({})", idx.include.join(", "))
        };
        s.push_str(&format!(
            "  - 索引：{} ON ({}){}{}\n",
            idx.name,
            idx.columns.join(", "),
            if idx.unique { " UNIQUE" } else { "" },
            inc
        ))
    }
    s
}

pub fn render_table_detail(t: &TableDetail) -> String {
    let mut s = String::new();
    s.push_str(&render_table_line(t));
    s
}
