use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Value,
}

impl Function {
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: None,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    pub fn with_parameters(mut self, parameters: Value) -> Self {
        self.parameters = parameters;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: Function,
}

impl Tool {
    pub fn new(function: Function) -> Self {
        Self {
            tool_type: "function".to_string(),
            function,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

impl FunctionCall {
    pub fn parse_arguments<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_str(&self.arguments)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCall,
}

pub fn create_function_tool(function: Function) -> Tool {
    Tool::new(function)
}

pub fn create_json_schema_properties(properties: Value) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": []
    })
}

impl From<rmcp::model::Tool> for Tool {
    fn from(rmcp_tool: rmcp::model::Tool) -> Self {
        // 将 input_schema (Arc<Map<String, Value>>) 转换为 Value
        let parameters = Value::Object(
            (*rmcp_tool.input_schema)
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );

        let function = Function {
            name: rmcp_tool.name.to_string(),
            description: rmcp_tool.description.map(|d| d.to_string()),
            parameters,
        };

        Tool {
            tool_type: "function".to_string(),
            function,
        }
    }
}

