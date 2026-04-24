use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type CorrelationId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FluxTraceContext {
    pub correlation_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchIntent {
    pub instance_id: String,
    pub version_id: String,
    pub username: String,
    pub uuid: String,
    pub token: String,
    pub acc_type: String,
    pub server_ip: Option<String>,
    pub world_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelixStageId {
    Admit,
    Plan,
    BootstrapPromote,
    ExecuteLaunch,
    ResolveProfiles,
    LegacyForgeUniversal,
    ClasspathAssemble,
    ClientLibrary,
    JavaRuntime,
    ProcessSpawn,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelixStage {
    pub id: HelixStageId,
    pub parallel_strands: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelixPlan {
    pub correlation_id: CorrelationId,
    pub stages: Vec<HelixStage>,
    pub estimated_io_weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClasspathSnapshot {
    pub key_hash: String,
    pub classpath_fingerprint: String,
    pub artifact_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchSpec {
    pub correlation_id: CorrelationId,
    pub instance_id: String,
    pub version_id: String,
    pub classpath: Option<ClasspathSnapshot>,
}

impl LaunchIntent {
    pub fn cache_invalidation_hint(&self) -> String {
        format!(
            "{}|{}|{}|{:?}|{:?}",
            self.instance_id, self.version_id, self.uuid, self.server_ip, self.world_name
        )
    }
}
