use crate::core::models::*;

pub fn render_er(db: &DatabaseSchema) -> String {
    let mut s = String::new();
    s.push_str("erDiagram\n");
    for sch in &db.schemas {
        for t in &sch.tables {
            s.push_str(&format!("  {}_{} {{\n", sch.name, t.name));
            for c in &t.columns {
                s.push_str(&format!("    {} {}\n", c.data_type, c.name));
            }
            s.push_str("  }\n");
        }
    }
    for sch in &db.schemas {
        for t in &sch.tables {
            for fk in &t.foreign_keys {
                // 显示为 多对一 关系
                s.push_str(&format!(
                    "  {}_{} }}o--|| {} : FK\n",
                    sch.name,
                    t.name,
                    fk.ref_table.replace('.', "_")
                ));
            }
        }
    }
    s
}
