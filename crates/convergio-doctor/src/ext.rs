//! Doctor extension — registers routes and provides manifest.

use axum::Router;
use convergio_db::pool::ConnPool;
use convergio_types::extension::{AppContext, Extension, Health, McpToolDef, Migration};
use convergio_types::manifest::{Capability, Manifest, ModuleKind};

use crate::runtime::DoctorRuntime;

pub struct DoctorExtension {
    pool: ConnPool,
    runtime: DoctorRuntime,
}

impl DoctorExtension {
    pub fn new(pool: ConnPool, runtime: DoctorRuntime) -> Self {
        Self { pool, runtime }
    }
}

impl Extension for DoctorExtension {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "convergio-doctor".into(),
            description: "System diagnostics and health checks".into(),
            version: crate::DOCTOR_VERSION.into(),
            kind: ModuleKind::Extension,
            provides: vec![Capability {
                name: "doctor".into(),
                version: crate::DOCTOR_VERSION.into(),
                description: "System diagnostics and health checks".into(),
            }],
            requires: vec![],
            agent_tools: vec![],
            required_roles: vec![],
        }
    }

    fn migrations(&self) -> Vec<Migration> {
        vec![Migration {
            version: 1,
            description: "doctor_reports table",
            up: "CREATE TABLE IF NOT EXISTS doctor_reports (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                version TEXT NOT NULL,
                daemon_version TEXT NOT NULL,
                total_checks INTEGER NOT NULL DEFAULT 0,
                passed INTEGER NOT NULL DEFAULT 0,
                warnings INTEGER NOT NULL DEFAULT 0,
                failed INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER NOT NULL DEFAULT 0,
                report_json TEXT NOT NULL DEFAULT '{}'
            );",
        }]
    }

    fn routes(&self, _ctx: &AppContext) -> Option<Router> {
        Some(crate::routes::doctor_routes(
            self.pool.clone(),
            self.runtime.clone(),
        ))
    }

    fn health(&self) -> Health {
        Health::Ok
    }

    fn mcp_tools(&self) -> Vec<McpToolDef> {
        crate::mcp_defs::doctor_tools()
    }
}
