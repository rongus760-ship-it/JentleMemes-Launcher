use super::types::HelixStageId;
use serde::Serialize;
use std::time::Instant;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PulseKind {
    StageStart,
    StageEnd,
}

#[derive(Debug, Clone, Serialize)]
pub struct PulsePayload {
    pub correlation_id: Uuid,
    pub stage: HelixStageId,
    pub kind: PulseKind,
    pub elapsed_ms: Option<u64>,
    pub message: Option<String>,
}

pub struct PulseSpan {
    stage: HelixStageId,
    correlation_id: Uuid,
    start: Instant,
}

impl PulseSpan {
    pub fn begin(
        app: &AppHandle,
        instance_id: &str,
        stage: HelixStageId,
        correlation_id: Uuid,
    ) -> Self {
        emit_stage_start(app, instance_id, correlation_id, stage);
        Self {
            stage,
            correlation_id,
            start: Instant::now(),
        }
    }

    pub fn finish(self, app: &AppHandle, instance_id: &str) {
        emit_pulse(
            app,
            instance_id,
            PulsePayload {
                correlation_id: self.correlation_id,
                stage: self.stage,
                kind: PulseKind::StageEnd,
                elapsed_ms: Some(self.start.elapsed().as_millis() as u64),
                message: None,
            },
        );
    }
}

pub fn emit_pulse(app: &AppHandle, instance_id: &str, payload: PulsePayload) {
    let _ = app.emit(&format!("pulse_{}", instance_id), payload);
}

pub fn emit_stage_start(
    app: &AppHandle,
    instance_id: &str,
    correlation_id: Uuid,
    stage: HelixStageId,
) {
    emit_pulse(
        app,
        instance_id,
        PulsePayload {
            correlation_id,
            stage,
            kind: PulseKind::StageStart,
            elapsed_ms: None,
            message: None,
        },
    );
}

pub fn emit_human_log(app: &AppHandle, instance_id: &str, line: &str) {
    let _ = app.emit(
        &format!("log_{}", instance_id),
        format!("[FluxCore] {}", line),
    );
}

pub struct FluxTraceScope {
    app: AppHandle,
    instance_id: String,
    correlation_id: Uuid,
    stage: HelixStageId,
    start: Instant,
    finished: bool,
}

impl FluxTraceScope {
    pub fn enter(
        app: &AppHandle,
        instance_id: &str,
        correlation_id: Uuid,
        stage: HelixStageId,
    ) -> Self {
        emit_stage_start(app, instance_id, correlation_id, stage);
        Self {
            app: app.clone(),
            instance_id: instance_id.to_string(),
            correlation_id,
            stage,
            start: Instant::now(),
            finished: false,
        }
    }

    fn emit_end(&self) {
        emit_pulse(
            &self.app,
            &self.instance_id,
            PulsePayload {
                correlation_id: self.correlation_id,
                stage: self.stage,
                kind: PulseKind::StageEnd,
                elapsed_ms: Some(self.start.elapsed().as_millis() as u64),
                message: None,
            },
        );
    }

    pub fn finish(mut self) {
        self.emit_end();
        self.finished = true;
    }
}

impl Drop for FluxTraceScope {
    fn drop(&mut self) {
        if !self.finished {
            self.emit_end();
            self.finished = true;
        }
    }
}
