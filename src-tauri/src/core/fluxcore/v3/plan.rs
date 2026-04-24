use super::types::{HelixPlan, HelixStage, HelixStageId, LaunchIntent};
use uuid::Uuid;

pub fn build_helix_plan(intent: &LaunchIntent, correlation_id: Uuid) -> HelixPlan {
    let mut strands_after_plan = 4u32;
    if intent.server_ip.is_none() && intent.world_name.is_none() {
        strands_after_plan = 2;
    }

    let stages = vec![
        HelixStage {
            id: HelixStageId::Admit,
            parallel_strands: 1,
        },
        HelixStage {
            id: HelixStageId::Plan,
            parallel_strands: 1,
        },
        HelixStage {
            id: HelixStageId::ExecuteLaunch,
            parallel_strands: strands_after_plan,
        },
    ];

    let estimated_io_weight = stages
        .iter()
        .map(|s| s.parallel_strands.max(1))
        .sum::<u32>()
        .saturating_mul(10);

    HelixPlan {
        correlation_id,
        stages,
        estimated_io_weight,
    }
}
