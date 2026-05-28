use crate::components::db_manager::PoolRegistry;
use egui_snarl::{
    InPin, InPinId, NodeId, OutPin, OutPinId, Snarl,
    ui::{
        BackgroundPattern, NodeLayout, PinInfo, PinPlacement, SnarlStyle, SnarlViewer, SnarlWidget,
        WireStyle,
    },
};
use pgone_sql::{ColumnDetail, ForeignKeyDetail, PrimaryKeyDetail, Session, TableDetail};
use poll_promise::Promise;
use std::collections::{HashMap, HashSet};

const TABLE_NODE_WIDTH: f32 = 280.0;
const TABLE_NODE_X_GAP: f32 = 340.0;
const TABLE_NODE_Y_GAP: f32 = 300.0;
const TABLE_NODES_PER_ROW: usize = 3;

#[derive(Clone, Debug)]
pub struct TableGraphNode {
    table: TableDetail,
    input_columns: Vec<String>,
    output_columns: Vec<String>,
}

impl TableGraphNode {
    fn input_column(&self, input: usize) -> Option<&str> {
        self.input_columns.get(input).map(String::as_str)
    }

    fn output_column(&self, output: usize) -> Option<&str> {
        self.output_columns.get(output).map(String::as_str)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphWire {
    pub from_table: usize,
    pub from_output: usize,
    pub to_table: usize,
    pub to_input: usize,
}

#[derive(Clone, Debug, Default)]
pub struct TableGraphModel {
    nodes: Vec<TableGraphNode>,
    wires: Vec<GraphWire>,
}

impl TableGraphModel {
    #[must_use]
    pub fn from_tables(tables: &[TableDetail]) -> Self {
        let table_indexes = tables
            .iter()
            .enumerate()
            .map(|(index, table)| (qualified_table_name(table), index))
            .collect::<HashMap<_, _>>();

        let mut input_columns = vec![HashSet::<String>::new(); tables.len()];
        let mut output_columns = vec![HashSet::<String>::new(); tables.len()];
        let mut raw_wires = Vec::new();

        for (source_index, table) in tables.iter().enumerate() {
            for fk in &table.foreign_keys {
                let Some(target_index) = referenced_table_index(&table_indexes, table, fk) else {
                    continue;
                };

                for (source_column, target_column) in fk.columns.iter().zip(fk.ref_columns.iter()) {
                    output_columns[source_index].insert(source_column.clone());
                    input_columns[target_index].insert(target_column.clone());
                    raw_wires.push((
                        source_index,
                        source_column.clone(),
                        target_index,
                        target_column.clone(),
                    ));
                }
            }
        }

        let nodes = tables
            .iter()
            .enumerate()
            .map(|(index, table)| TableGraphNode {
                table: table.clone(),
                input_columns: sorted_columns(&table.columns, &input_columns[index]),
                output_columns: sorted_columns(&table.columns, &output_columns[index]),
            })
            .collect::<Vec<_>>();

        let input_indexes = nodes
            .iter()
            .enumerate()
            .flat_map(|(table_index, node)| {
                node.input_columns
                    .iter()
                    .enumerate()
                    .map(move |(pin_index, column)| ((table_index, column.clone()), pin_index))
            })
            .collect::<HashMap<_, _>>();
        let output_indexes = nodes
            .iter()
            .enumerate()
            .flat_map(|(table_index, node)| {
                node.output_columns
                    .iter()
                    .enumerate()
                    .map(move |(pin_index, column)| ((table_index, column.clone()), pin_index))
            })
            .collect::<HashMap<_, _>>();

        let wires = raw_wires
            .into_iter()
            .filter_map(|(from_table, from_column, to_table, to_column)| {
                match (
                    output_indexes.get(&(from_table, from_column)),
                    input_indexes.get(&(to_table, to_column)),
                ) {
                    (Some(&from_output), Some(&to_input)) => Some(GraphWire {
                        from_table,
                        from_output,
                        to_table,
                        to_input,
                    }),
                    _ => None,
                }
            })
            .collect();

        Self { nodes, wires }
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn wire_count(&self) -> usize {
        self.wires.len()
    }

    fn into_snarl(self) -> Snarl<TableGraphNode> {
        let mut snarl = Snarl::new();
        let mut node_ids = Vec::with_capacity(self.nodes.len());

        for (index, node) in self.nodes.into_iter().enumerate() {
            let row = index / TABLE_NODES_PER_ROW;
            let col = index % TABLE_NODES_PER_ROW;
            let pos = egui::pos2(
                (col as f32) * TABLE_NODE_X_GAP,
                (row as f32) * TABLE_NODE_Y_GAP,
            );
            node_ids.push(snarl.insert_node(pos, node));
        }

        for wire in self.wires {
            let Some(&from_node) = node_ids.get(wire.from_table) else {
                continue;
            };
            let Some(&to_node) = node_ids.get(wire.to_table) else {
                continue;
            };
            snarl.connect(
                OutPinId {
                    node: from_node,
                    output: wire.from_output,
                },
                InPinId {
                    node: to_node,
                    input: wire.to_input,
                },
            );
        }

        snarl
    }
}

pub struct SchemaGraph {
    tables: Vec<TableDetail>,
    schema_name: String,
    database_name: String,
    loading: bool,
    error: Option<String>,
    promise: Option<Promise<Result<Vec<TableDetail>, String>>>,
    initialized: bool,
    snarl: Snarl<TableGraphNode>,
    needs_rebuild: bool,
}

impl Default for SchemaGraph {
    fn default() -> Self {
        Self {
            tables: Vec::new(),
            schema_name: String::new(),
            database_name: String::new(),
            loading: false,
            error: None,
            promise: None,
            initialized: false,
            snarl: Snarl::new(),
            needs_rebuild: false,
        }
    }
}

impl SchemaGraph {
    pub fn new(database_name: String, schema_name: String) -> Self {
        Self {
            tables: Vec::new(),
            schema_name,
            database_name,
            loading: false,
            error: None,
            promise: None,
            initialized: false,
            snarl: Snarl::new(),
            needs_rebuild: false,
        }
    }

    pub fn load_data(&mut self, pools: PoolRegistry, dsn: &str) {
        if self.loading || self.promise.is_some() {
            return;
        }

        self.loading = true;
        self.error = None;
        let schema_name = self.schema_name.clone();
        let dsn = dsn.to_string();

        let (sender, promise) = Promise::new();
        self.promise = Some(promise);

        crate::futures::spawn(async move {
            let result: Result<Vec<TableDetail>, String> = async move {
                let pool = pools.get_or_create_pool(&dsn).await?;
                let session = Session::from_pool(pool);
                session
                    .list_table_details(&schema_name)
                    .await
                    .map_err(|e| e.to_string())
            }
            .await;

            sender.send(result);
        });
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, pools: PoolRegistry, dsn: Option<&str>) {
        if let Some(ref promise) = self.promise
            && let Some(result) = promise.ready()
        {
            self.loading = false;
            match result {
                Ok(tables) => {
                    self.tables = tables.clone();
                    self.initialized = true;
                    self.needs_rebuild = true;
                }
                Err(e) => {
                    self.error = Some(e.clone());
                }
            }
            self.promise = None;
        }

        if !self.initialized && dsn.is_some() && self.promise.is_none() && !self.loading {
            self.load_data(pools, dsn.unwrap());
        }

        if self.loading {
            ui.centered_and_justified(|ui| {
                ui.spinner();
                ui.label("Loading table information...");
            });
            return;
        }

        if let Some(err) = &self.error {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
            if ui.button("Retry").clicked() {
                self.error = None;
                self.initialized = false;
                self.needs_rebuild = false;
                self.snarl = Snarl::new();
            }
            return;
        }

        if self.tables.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label("No tables found");
            });
            return;
        }

        if self.needs_rebuild {
            self.snarl = TableGraphModel::from_tables(&self.tables).into_snarl();
            self.needs_rebuild = false;
        }

        self.show_schema_graph(ui);
    }

    fn show_schema_graph(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(format!("{}.{}", self.database_name, self.schema_name));
            ui.label(
                egui::RichText::new(format!(
                    "{} tables, {} relationships",
                    self.tables.len(),
                    self.tables
                        .iter()
                        .map(|table| table.foreign_keys.len())
                        .sum::<usize>()
                ))
                .small()
                .weak(),
            );
        });
        ui.separator();

        let style = snarl_style(ui);
        let mut viewer = TableGraphViewer;
        SnarlWidget::new()
            .id(egui::Id::new((
                "schema_graph",
                self.database_name.as_str(),
                self.schema_name.as_str(),
            )))
            .style(style)
            .min_size(ui.available_size())
            .show(&mut self.snarl, &mut viewer, ui);
    }
}

struct TableGraphViewer;

impl SnarlViewer<TableGraphNode> for TableGraphViewer {
    fn title(&mut self, node: &TableGraphNode) -> String {
        node.table.name.clone()
    }

    fn inputs(&mut self, node: &TableGraphNode) -> usize {
        node.input_columns.len()
    }

    #[allow(refining_impl_trait)]
    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<TableGraphNode>,
    ) -> PinInfo {
        let column = snarl[pin.id.node].input_column(pin.id.input).unwrap_or("");
        ui.label(egui::RichText::new(column).monospace().small());
        PinInfo::circle()
            .with_fill(egui::Color32::from_rgb(75, 168, 120))
            .with_wire_color(egui::Color32::from_rgb(75, 168, 120))
            .with_wire_style(WireStyle::AxisAligned { corner_radius: 8.0 })
    }

    fn outputs(&mut self, node: &TableGraphNode) -> usize {
        node.output_columns.len()
    }

    #[allow(refining_impl_trait)]
    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        snarl: &mut Snarl<TableGraphNode>,
    ) -> PinInfo {
        let column = snarl[pin.id.node]
            .output_column(pin.id.output)
            .unwrap_or("");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(column).monospace().small());
        });
        PinInfo::circle()
            .with_fill(egui::Color32::from_rgb(245, 159, 52))
            .with_wire_color(egui::Color32::from_rgb(245, 159, 52))
            .with_wire_style(WireStyle::AxisAligned { corner_radius: 8.0 })
    }

    fn has_body(&mut self, _node: &TableGraphNode) -> bool {
        true
    }

    fn show_body(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<TableGraphNode>,
    ) {
        let node = &snarl[node];
        ui.set_min_width(TABLE_NODE_WIDTH);
        ui.vertical(|ui| {
            if let Some(comment) = &node.table.comment {
                ui.label(egui::RichText::new(comment).small().italics().weak());
                ui.separator();
            }

            for column in &node.table.columns {
                show_column_row(ui, &node.table, column);
            }
        });
    }

    fn header_frame(
        &mut self,
        frame: egui::Frame,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        snarl: &Snarl<TableGraphNode>,
    ) -> egui::Frame {
        frame.fill(table_color(&snarl[node].table.name))
    }

    fn show_header(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        snarl: &mut Snarl<TableGraphNode>,
    ) {
        ui.horizontal(|ui| {
            ui.label(egui_phosphor::regular::TABLE);
            ui.strong(&snarl[node].table.name);
        });
    }

    fn connect(&mut self, _from: &OutPin, _to: &InPin, _snarl: &mut Snarl<TableGraphNode>) {}

    fn disconnect(&mut self, _from: &OutPin, _to: &InPin, _snarl: &mut Snarl<TableGraphNode>) {}

    fn drop_outputs(&mut self, _pin: &OutPin, _snarl: &mut Snarl<TableGraphNode>) {}

    fn drop_inputs(&mut self, _pin: &InPin, _snarl: &mut Snarl<TableGraphNode>) {}
}

fn show_column_row(ui: &mut egui::Ui, table: &TableDetail, column: &ColumnDetail) {
    ui.horizontal(|ui| {
        ui.set_min_width(TABLE_NODE_WIDTH - 24.0);
        if is_primary_key(&table.primary_key, &column.name) {
            ui.label(
                egui::RichText::new(egui_phosphor::regular::KEY)
                    .color(egui::Color32::from_rgb(245, 159, 52)),
            );
        } else {
            ui.add_space(14.0);
        }

        if is_foreign_key(&table.foreign_keys, &column.name) {
            ui.label(
                egui::RichText::new(egui_phosphor::regular::LINK)
                    .color(egui::Color32::from_rgb(75, 168, 120)),
            );
        } else {
            ui.add_space(14.0);
        }

        ui.label(
            egui::RichText::new(&column.name)
                .monospace()
                .small()
                .strong(),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut details = column.data_type.clone();
            if !column.nullable {
                details.push_str(" not null");
            }
            if column.default.is_some() {
                details.push_str(" default");
            }
            ui.label(egui::RichText::new(details).small().weak());
        });
    });
}

fn snarl_style(ui: &egui::Ui) -> SnarlStyle {
    let visuals = ui.visuals();
    SnarlStyle {
        node_layout: Some(NodeLayout::coil().with_min_pin_row_height(20.0)),
        pin_placement: Some(PinPlacement::Edge),
        pin_size: Some(8.0),
        wire_width: Some(2.0),
        wire_frame_size: Some(36.0),
        bg_pattern: Some(BackgroundPattern::grid(egui::vec2(24.0, 24.0), 0.0)),
        bg_pattern_stroke: Some(egui::Stroke::new(
            1.0,
            visuals
                .widgets
                .noninteractive
                .bg_stroke
                .color
                .gamma_multiply(0.35),
        )),
        node_frame: Some(egui::Frame {
            inner_margin: egui::Margin::same(8),
            outer_margin: egui::Margin {
                left: 0,
                right: 0,
                top: 0,
                bottom: 4,
            },
            corner_radius: egui::CornerRadius::same(8),
            fill: visuals.window_fill(),
            stroke: visuals.widgets.noninteractive.bg_stroke,
            shadow: egui::Shadow::NONE,
        }),
        header_frame: Some(egui::Frame {
            inner_margin: egui::Margin::symmetric(8, 6),
            outer_margin: egui::Margin::ZERO,
            corner_radius: egui::CornerRadius {
                nw: 8,
                ne: 8,
                sw: 0,
                se: 0,
            },
            fill: visuals.selection.bg_fill,
            stroke: egui::Stroke::NONE,
            shadow: egui::Shadow::NONE,
        }),
        collapsible: Some(false),
        min_scale: Some(0.35),
        max_scale: Some(1.6),
        ..SnarlStyle::new()
    }
}

fn referenced_table_index(
    table_indexes: &HashMap<String, usize>,
    source_table: &TableDetail,
    fk: &ForeignKeyDetail,
) -> Option<usize> {
    if let Some(index) = table_indexes.get(&fk.ref_table) {
        return Some(*index);
    }

    if fk.ref_table.contains('.') {
        return None;
    }

    table_indexes
        .get(&format!("{}.{}", source_table.schema, fk.ref_table))
        .copied()
}

fn qualified_table_name(table: &TableDetail) -> String {
    format!("{}.{}", table.schema, table.name)
}

fn sorted_columns(columns: &[ColumnDetail], selected: &HashSet<String>) -> Vec<String> {
    columns
        .iter()
        .filter(|column| selected.contains(&column.name))
        .map(|column| column.name.clone())
        .collect()
}

fn is_primary_key(primary_key: &Option<PrimaryKeyDetail>, column: &str) -> bool {
    primary_key
        .as_ref()
        .is_some_and(|primary_key| primary_key.columns.iter().any(|pk| pk == column))
}

fn is_foreign_key(foreign_keys: &[ForeignKeyDetail], column: &str) -> bool {
    foreign_keys.iter().any(|foreign_key| {
        foreign_key
            .columns
            .iter()
            .any(|fk_column| fk_column == column)
    })
}

fn table_color(table_name: &str) -> egui::Color32 {
    let bytes = table_name.as_bytes();
    let hash = bytes
        .iter()
        .fold(0_u32, |hash, byte| hash.wrapping_mul(31) + u32::from(*byte));
    let hue = (hash % 360) as f32 / 360.0;
    egui::ecolor::Hsva::new(hue, 0.45, 0.62, 1.0).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column(name: &str) -> ColumnDetail {
        ColumnDetail {
            name: name.to_string(),
            data_type: "integer".to_string(),
            udt_name: None,
            nullable: false,
            default: None,
            character_maximum_length: None,
            numeric_precision: None,
            numeric_scale: None,
            comment: None,
        }
    }

    fn table(name: &str, columns: &[&str], foreign_keys: Vec<ForeignKeyDetail>) -> TableDetail {
        TableDetail {
            schema: "public".to_string(),
            name: name.to_string(),
            comment: None,
            columns: columns.iter().map(|name| column(name)).collect(),
            primary_key: Some(PrimaryKeyDetail {
                columns: vec!["id".to_string()],
            }),
            foreign_keys,
        }
    }

    fn foreign_key(columns: &[&str], ref_table: &str, ref_columns: &[&str]) -> ForeignKeyDetail {
        ForeignKeyDetail {
            columns: columns.iter().map(|column| column.to_string()).collect(),
            ref_table: ref_table.to_string(),
            ref_columns: ref_columns
                .iter()
                .map(|column| column.to_string())
                .collect(),
            on_update: None,
            on_delete: None,
        }
    }

    #[test]
    fn graph_model_creates_one_node_per_table() {
        let model = TableGraphModel::from_tables(&[
            table("users", &["id"], Vec::new()),
            table("orders", &["id"], Vec::new()),
        ]);

        assert_eq!(model.node_count(), 2);
    }

    #[test]
    fn graph_model_creates_wire_for_foreign_key() {
        let model = TableGraphModel::from_tables(&[
            table("users", &["id"], Vec::new()),
            table(
                "orders",
                &["id", "user_id"],
                vec![foreign_key(&["user_id"], "public.users", &["id"])],
            ),
        ]);

        assert_eq!(model.wire_count(), 1);
        assert_eq!(
            model.wires[0],
            GraphWire {
                from_table: 1,
                from_output: 0,
                to_table: 0,
                to_input: 0,
            }
        );
    }

    #[test]
    fn graph_model_maps_composite_foreign_keys() {
        let model = TableGraphModel::from_tables(&[
            table("parents", &["tenant_id", "id"], Vec::new()),
            table(
                "children",
                &["id", "tenant_id", "parent_id"],
                vec![foreign_key(
                    &["tenant_id", "parent_id"],
                    "public.parents",
                    &["tenant_id", "id"],
                )],
            ),
        ]);

        assert_eq!(model.wire_count(), 2);
        assert_eq!(
            model.nodes[1].output_columns,
            vec!["tenant_id", "parent_id"]
        );
        assert_eq!(model.nodes[0].input_columns, vec!["tenant_id", "id"]);
    }

    #[test]
    fn graph_model_skips_missing_referenced_tables() {
        let model = TableGraphModel::from_tables(&[table(
            "orders",
            &["id", "user_id"],
            vec![foreign_key(&["user_id"], "public.users", &["id"])],
        )]);

        assert_eq!(model.node_count(), 1);
        assert_eq!(model.wire_count(), 0);
    }
}
