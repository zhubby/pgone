use crate::core::models::*;

pub fn render_dbml(db: &DatabaseSchema) -> String {
    let mut s = String::new();
    for sch in &db.schemas {
        for t in &sch.tables {
            s.push_str(&format!("Table {}.{} {{\n", sch.name, t.name));
            for c in &t.columns {
                let null = if c.nullable { "" } else { " [not null]" };
                s.push_str(&format!("  {} {}{},\n", c.name, c.data_type, null));
            }
            if let Some(pk) = &t.primary_key { s.push_str(&format!("  indexes {{\n    ({} ) [pk]\n  }}\n", pk.columns.join(", "))); }
            s.push_str("}\n\n");
        }
    }
    for sch in &db.schemas {
        for t in &sch.tables {
            for fk in &t.foreign_keys {
                s.push_str(&format!("Ref: {}.{} .({}) > {} .({})\n",
                    sch.name, t.name, fk.columns.join(", "), fk.ref_table, fk.ref_columns.join(", ")));
            }
        }
    }
    s
}


