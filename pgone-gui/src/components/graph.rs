use egui_snarl::{InPinId, NodeId, OutPinId, Snarl};
use egui_snarl::ui::{SnarlStyle, SnarlViewer};
use pgone_sql::{Session, TableDetail};
use poll_promise::Promise;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TableNode {
    pub table_name: String,
    pub color: egui::Color32,
}

impl TableNode {
    fn new(table_name: String) -> Self {
        let mut hasher = DefaultHasher::new();
        table_name.hash(&mut hasher);
        let hash = hasher.finish();
        
        // Generate a color based on hash - use RGB
        let r = ((hash >> 0) & 0xFF) as u8;
        let g = ((hash >> 8) & 0xFF) as u8;
        let b = ((hash >> 16) & 0xFF) as u8;
        // Ensure minimum brightness
        let r = r.max(100);
        let g = g.max(100);
        let b = b.max(100);
        let color = egui::Color32::from_rgb(r, g, b);
        
        Self { table_name, color }
    }
}

pub struct SchemaGraphViewer {
    tables: Vec<TableDetail>,
    table_map: HashMap<String, usize>, // table_name -> index
}

impl SnarlViewer<TableNode> for SchemaGraphViewer {
    fn title(&mut self, node: &TableNode) -> String {
        node.table_name.clone()
    }

    fn inputs(&mut self, node: &TableNode) -> usize {
        let table_idx = match self.table_map.get(&node.table_name) {
            Some(idx) => *idx,
            None => return 0,
        };
        let table = &self.tables[table_idx];
        
        // Count foreign key columns as inputs
        table.foreign_keys.iter().map(|fk| fk.columns.len()).sum()
    }

    fn outputs(&mut self, node: &TableNode) -> usize {
        let table_idx = match self.table_map.get(&node.table_name) {
            Some(idx) => *idx,
            None => return 0,
        };
        let table = &self.tables[table_idx];
        
        // Count primary key columns as outputs
        table.primary_key.as_ref().map(|pk| pk.columns.len()).unwrap_or(0)
    }

    fn show_input(
        &mut self,
        pin: &egui_snarl::InPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<TableNode>,
    ) -> impl egui_snarl::ui::SnarlPin + 'static {
        // Find the column for this pin
        let node_data = snarl.get_node(pin.id.node).map(|n| n.clone());
        if let Some(node) = node_data {
            let table_idx = match self.table_map.get(&node.table_name) {
                Some(idx) => *idx,
                None => return egui_snarl::ui::PinInfo::default(),
            };
            let table = &self.tables[table_idx];
            
            // Find which foreign key column this pin represents
            let mut pin_idx = 0;
            for fk in &table.foreign_keys {
                if pin_idx + fk.columns.len() > pin.id.input {
                    let col_idx = pin.id.input - pin_idx;
                    if let Some(col_name) = fk.columns.get(col_idx) {
                        if let Some(col) = table.columns.iter().find(|c| c.name == *col_name) {
                            let mut text = col.name.clone();
                            text.push_str(": ");
                            text.push_str(&col.data_type);
                            if !col.nullable {
                                text.push_str(" NOT NULL");
                            }
                            text.push_str(" [FK]");
                            
                            ui.label(text);
                            if let Some(comment) = &col.comment {
                                ui.small(comment);
                            }
                        }
                    }
                    break;
                }
                pin_idx += fk.columns.len();
            }
        }
        egui_snarl::ui::PinInfo::default()
    }

    fn show_output(
        &mut self,
        pin: &egui_snarl::OutPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<TableNode>,
    ) -> impl egui_snarl::ui::SnarlPin + 'static {
        // Find the column for this pin
        let node_data = snarl.get_node(pin.id.node).map(|n| n.clone());
        if let Some(node) = node_data {
            let table_idx = match self.table_map.get(&node.table_name) {
                Some(idx) => *idx,
                None => return egui_snarl::ui::PinInfo::default(),
            };
            let table = &self.tables[table_idx];
            
            // Find which primary key column this pin represents
            if let Some(pk) = &table.primary_key {
                if let Some(col_name) = pk.columns.get(pin.id.output) {
                    if let Some(col) = table.columns.iter().find(|c| c.name == *col_name) {
                        let mut text = col.name.clone();
                        text.push_str(": ");
                        text.push_str(&col.data_type);
                        if !col.nullable {
                            text.push_str(" NOT NULL");
                        }
                        text.push_str(" [PK]");
                        
                        ui.label(text);
                        if let Some(comment) = &col.comment {
                            ui.small(comment);
                        }
                    }
                }
            }
        }
        egui_snarl::ui::PinInfo::default()
    }

    fn has_body(&mut self, _node: &TableNode) -> bool {
        true
    }

    fn show_body(
        &mut self,
        node_id: NodeId,
        _inputs: &[egui_snarl::InPin],
        _outputs: &[egui_snarl::OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<TableNode>,
    ) {
        let node_data = snarl.get_node(node_id).map(|n| n.clone());
        if let Some(node) = node_data {
            let table_idx = match self.table_map.get(&node.table_name) {
                Some(idx) => *idx,
                None => return,
            };
            let table = &self.tables[table_idx];
            
            ui.vertical(|ui| {
                // Show table comment if available
                if let Some(comment) = &table.comment {
                    ui.label(egui::RichText::new(comment).italics());
                    ui.separator();
                }
                
                // Show all columns
                for col in &table.columns {
                    let mut text = col.name.clone();
                    text.push_str(": ");
                    text.push_str(&col.data_type);
                    
                    if !col.nullable {
                        text.push_str(" NOT NULL");
                    }
                    
                    // Add primary key indicator
                    if let Some(pk) = &table.primary_key {
                        if pk.columns.contains(&col.name) {
                            text.push_str(" [PK]");
                        }
                    }
                    
                    // Add foreign key indicator
                    for fk in &table.foreign_keys {
                        if fk.columns.contains(&col.name) {
                            text.push_str(" [FK→");
                            text.push_str(&fk.ref_table);
                            text.push(']');
                            break;
                        }
                    }
                    
                    ui.horizontal(|ui| {
                        ui.label(text);
                        if let Some(comment) = &col.comment {
                            ui.label(egui::RichText::new(format!("({})", comment)).small().weak());
                        }
                    });
                }
            });
        }
    }

    fn has_node_style(&mut self, _node_id: NodeId, _inputs: &[egui_snarl::InPin], _outputs: &[egui_snarl::OutPin], _snarl: &Snarl<TableNode>) -> bool {
        true
    }

    fn apply_node_style(
        &mut self,
        style: &mut egui::Style,
        node_id: NodeId,
        _inputs: &[egui_snarl::InPin],
        _outputs: &[egui_snarl::OutPin],
        snarl: &Snarl<TableNode>,
    ) {
        if let Some(node) = snarl.get_node(node_id) {
            // Apply node color - note: this might not work as expected, 
            // we may need to use a different approach for coloring nodes
            style.visuals.widgets.noninteractive.bg_fill = node.color;
        }
    }
}

pub struct SchemaGraph {
    snarl: Snarl<TableNode>,
    tables: Vec<TableDetail>,
    schema_name: String,
    database_name: String,
    loading: bool,
    error: Option<String>,
    promise: Option<Promise<Result<Vec<TableDetail>, String>>>,
    initialized: bool,
}

impl Default for SchemaGraph {
    fn default() -> Self {
        Self {
            snarl: Snarl::default(),
            tables: Vec::new(),
            schema_name: String::new(),
            database_name: String::new(),
            loading: false,
            error: None,
            promise: None,
            initialized: false,
        }
    }
}

impl SchemaGraph {
    pub fn new(database_name: String, schema_name: String) -> Self {
        Self {
            snarl: Snarl::default(),
            tables: Vec::new(),
            schema_name,
            database_name,
            loading: false,
            error: None,
            promise: None,
            initialized: false,
        }
    }

    pub fn load_data(&mut self, dsn: &str) {
        if self.loading || self.promise.is_some() {
            return;
        }

        self.loading = true;
        self.error = None;
        let schema_name = self.schema_name.clone();
        let dsn = dsn.to_string();

        self.promise = Some(Promise::spawn_thread("load_table_details", move || {
            crate::futures::block_on_async(async move {
                let session = Session::new(&dsn).await.map_err(|e| e.to_string())?;
                session
                    .list_table_details(&schema_name)
                    .await
                    .map_err(|e| e.to_string())
            })
        }));
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, dsn: Option<&str>) {
        // Check promise status
        if let Some(promise) = &self.promise {
            if let Some(result) = promise.ready() {
                self.loading = false;
                match result {
                    Ok(tables) => {
                        self.tables = tables.clone();
                        self.initialize_graph();
                    }
                    Err(e) => {
                        self.error = Some(e.clone());
                    }
                }
                self.promise = None;
            }
        }

        // Start loading if needed
        if !self.initialized && dsn.is_some() && self.promise.is_none() && !self.loading {
            self.load_data(dsn.unwrap());
        }

        if self.loading {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.label("加载表信息中...");
            });
            return;
        }

        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("错误: {}", err));
            if ui.button("重试").clicked() {
                self.error = None;
                self.initialized = false;
            }
            return;
        }

        if self.tables.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("没有找到表");
            });
            return;
        }

        // Render the graph
        let mut viewer = SchemaGraphViewer {
            tables: self.tables.clone(),
            table_map: self
                .tables
                .iter()
                .enumerate()
                .map(|(i, t)| (t.name.clone(), i))
                .collect(),
        };

        let style = SnarlStyle::default();
        self.snarl.show(&mut viewer, &style, "schema_graph", ui);
    }

    fn initialize_graph(&mut self) {
        if self.initialized {
            return;
        }

        // Create nodes for each table
        for table in &self.tables {
            let node = TableNode::new(table.name.clone());
            
            // Set initial position in a grid layout
            let table_idx = self
                .tables
                .iter()
                .position(|t| t.name == table.name)
                .unwrap();
            let cols = (self.tables.len() as f32).sqrt().ceil() as usize;
            let row = table_idx / cols;
            let col = table_idx % cols;
            
            let x = (col as f32) * 300.0 + 100.0;
            let y = (row as f32) * 200.0 + 100.0;
            
            let _node_id = self.snarl.insert_node(egui::Pos2::new(x, y), node);
        }

        // Create wires for foreign keys
        for table in &self.tables {
            // Find the table node
            let table_node_id = self
                .snarl
                .nodes_ids_data()
                .find(|(_, node)| node.value.table_name == table.name)
                .map(|(id, _)| id);

            if let Some(from_node_id) = table_node_id {
                for fk in &table.foreign_keys {
                    // Find the referenced table node
                    let ref_table_node_id = self
                        .snarl
                        .nodes_ids_data()
                        .find(|(_, node)| node.value.table_name == fk.ref_table)
                        .map(|(id, _)| id);

                    if let Some(to_node_id) = ref_table_node_id {
                        // Connect each foreign key column to the corresponding primary key column
                        for (i, fk_col) in fk.columns.iter().enumerate() {
                            if let Some(ref_col) = fk.ref_columns.get(i) {
                                // Find input pin index (foreign key column)
                                let mut input_pin_idx = 0;
                                for fk2 in &table.foreign_keys {
                                    if fk2.columns.contains(fk_col) {
                                        if let Some(pos) = fk2.columns.iter().position(|c| c == fk_col) {
                                            input_pin_idx += pos;
                                            break;
                                        }
                                    }
                                    input_pin_idx += fk2.columns.len();
                                }
                                
                                // Find output pin index (primary key column in referenced table)
                                let ref_table = self.tables.iter().find(|t| t.name == fk.ref_table);
                                let output_pin_idx = ref_table
                                    .and_then(|t| t.primary_key.as_ref())
                                    .and_then(|pk| pk.columns.iter().position(|c| c == ref_col));

                                if let Some(output_idx) = output_pin_idx {
                                    let input_pin = InPinId {
                                        node: from_node_id,
                                        input: input_pin_idx,
                                    };
                                    let output_pin = OutPinId {
                                        node: to_node_id,
                                        output: output_idx,
                                    };
                                    let _ = self.snarl.connect(output_pin, input_pin);
                                }
                            }
                        }
                    }
                }
            }
        }

        self.initialized = true;
    }
}
