//! MCP tool definitions for the doctor extension.

use convergio_types::extension::McpToolDef;
use serde_json::json;

pub fn doctor_tools() -> Vec<McpToolDef> {
    vec![McpToolDef {
        name: "cvg_doctor_run".into(),
        description: "Run doctor diagnostics and return health report.".into(),
        method: "POST".into(),
        path: "/api/doctor/run".into(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "checks": {"type": "array", "items": {"type": "string"},
                    "description": "Specific checks to run (empty = all)"}
            }
        }),
        min_ring: "community".into(),
        path_params: vec![],
    }]
}
