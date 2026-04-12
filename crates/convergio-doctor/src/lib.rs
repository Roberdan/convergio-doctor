//! convergio-doctor — system diagnostics, health checks, and stress testing.
//!
//! Provides `/api/doctor` endpoints for runtime health verification.
//! Designed as the "doctor" pattern (like `flutter doctor` or `brew doctor`).
//!
//! Doctor v1.1 check categories:
//! - **core**: DB connectivity, migrations, config, disk
//! - **extensions**: dep graph, scheduled tasks, health status
//! - **api**: endpoint reachability (from CLI side)
//! - **stress**: concurrent operation safety (optional)

pub mod check_advanced;
pub mod check_beta;
pub mod check_beta_mcp;
pub mod check_beta_skills;
pub mod check_chaos_daemon;
pub mod check_chaos_db;
pub mod check_chaos_net;
pub mod check_cleanup;
pub mod check_command_sync;
pub mod check_config;
pub mod check_e2e_cleanup;
pub mod check_e2e_gates;
pub mod check_e2e_helpers;
pub mod check_e2e_ipc;
pub mod check_e2e_mesh;
pub mod check_e2e_org;
pub mod check_e2e_org_flow;
pub mod check_e2e_plan;
pub mod check_e2e_security;
pub mod check_e2e_skill_routes;
pub mod check_e2e_smoke;
pub mod check_e2e_smoke_ext;
pub mod check_e2e_wave_gate;
pub mod check_integrity;
pub mod check_projects;
pub mod check_spec_compliance;
pub mod checks;
pub mod ext;
pub mod mcp_defs;
pub mod routes;
pub mod runtime;

pub use ext::DoctorExtension;
pub use runtime::DoctorRuntime;

/// Doctor check suite version — independent of daemon version.
pub const DOCTOR_VERSION: &str = env!("CARGO_PKG_VERSION");
