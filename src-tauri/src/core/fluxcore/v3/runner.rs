use super::bus::{emit_human_log, PulseSpan};
use super::coil;
use super::gate;
use super::plan::build_helix_plan;
use super::types::{FluxTraceContext, HelixStageId, LaunchIntent, LaunchSpec};
use crate::core::game::launch;
use crate::error::Result;
use tauri::AppHandle;
use uuid::Uuid;

/// Запуск игры. Конвейер условный DAG: admit → plan → execute.
/// На стадии `plan` параллельно через `try_join!` резолвятся входы, которые не зависят друг от друга:
/// - mtime-digest цепочки профиля (Coil),
/// - поиск предыдущего snapshot в файле `<game_dir>/.fluxcore/coil_snapshot.json`.
/// Настоящая параллелизация тяжёлых шагов (Java/Classpath/Natives) делается уже внутри
/// `game::launch::launch` с помощью `LaunchCache`; после Phase 3 (декомпозиция launch.rs)
/// они будут вынесены в отдельные futures и объединены здесь полноценным DAG.
pub async fn run_game_launch(app: AppHandle, intent: LaunchIntent) -> Result<String> {
    let correlation_id = Uuid::new_v4();
    let flux = FluxTraceContext { correlation_id };
    let runner_t0 = std::time::Instant::now();

    emit_human_log(
        &app,
        &intent.instance_id,
        "▸ FluxCore v3: подготовка к запуску...",
    );

    let admit = PulseSpan::begin(
        &app,
        &intent.instance_id,
        HelixStageId::Admit,
        correlation_id,
    );
    let _slot = gate::try_acquire_exclusive_owned(&intent.instance_id).await?;

    if launch::running_instance_ids().contains(&intent.instance_id) {
        return Err(crate::error::Error::Custom(
            "Сборка уже запущена.".into(),
        ));
    }
    admit.finish(&app, &intent.instance_id);

    let plan_span = PulseSpan::begin(
        &app,
        &intent.instance_id,
        HelixStageId::Plan,
        correlation_id,
    );
    let helix = build_helix_plan(&intent, correlation_id);
    let _parallel = super::strand::suggested_parallel_strands(helix.estimated_io_weight);

    let data_dir = crate::config::get_data_dir();
    let game_dir = data_dir.join("instances").join(&intent.instance_id);

    // Параллельный резолв входов стадии plan: digest цепочки + поиск snapshot. Оба не зависят друг от друга.
    let (digest_res, snapshot_res) = {
        let data_dir_for_digest = data_dir.clone();
        let version_id_for_digest = intent.version_id.clone();
        let game_dir_for_snapshot = game_dir.clone();
        tokio::join!(
            async move {
                tokio::task::spawn_blocking(move || {
                    coil::profile_inputs_digest(&data_dir_for_digest, &version_id_for_digest)
                })
                .await
                .ok()
                .and_then(|r| r.ok())
            },
            async move {
                tokio::task::spawn_blocking(move || coil::load_snapshot(&game_dir_for_snapshot))
                    .await
                    .ok()
                    .flatten()
            }
        )
    };

    let snapshot_hint = match (digest_res.as_ref(), snapshot_res.as_ref()) {
        (Some(d), Some(prev))
            if prev.inputs_digest == *d && prev.version_id == intent.version_id =>
        {
            emit_human_log(
                &app,
                &intent.instance_id,
                "▸ CoilCache: цепочка профилей совпадает со snapshot (mtime hash).",
            );
            Some(super::types::ClasspathSnapshot {
                key_hash: d.clone(),
                classpath_fingerprint: prev.classpath_fingerprint.clone(),
                artifact_count: prev.artifact_count,
            })
        }
        _ => None,
    };

    let spec = LaunchSpec {
        correlation_id,
        instance_id: intent.instance_id.clone(),
        version_id: intent.version_id.clone(),
        classpath: snapshot_hint.clone(),
    };
    plan_span.finish(&app, &intent.instance_id);
    let plan_ms = runner_t0.elapsed().as_millis() as u64;
    emit_human_log(
        &app,
        &intent.instance_id,
        &format!("⏱ runner plan (digest+snapshot) Δ={plan_ms}ms"),
    );

    emit_human_log(
        &app,
        &intent.instance_id,
        "▸ Запуск через launch engine (Helix + Coil + Strand)...",
    );

    let exec = PulseSpan::begin(
        &app,
        &intent.instance_id,
        HelixStageId::ExecuteLaunch,
        correlation_id,
    );
    let out = launch::launch(
        app.clone(),
        &intent.instance_id,
        &intent.version_id,
        &intent.username,
        &intent.uuid,
        &intent.token,
        &intent.acc_type,
        intent.server_ip.as_deref(),
        intent.world_name.as_deref(),
        Some(flux),
        spec.classpath.clone(),
    )
    .await?;
    exec.finish(&app, &intent.instance_id);

    Ok(out)
}
