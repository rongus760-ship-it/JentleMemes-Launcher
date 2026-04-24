//! FluxCore — конвейер запуска игры.
//!
//! В этом подмодуле часть «resolver»-ов пока подключены как scaffolding для Phase 3
//! (decomposition of `game::launch` в отдельные futures), и ряд утилит зарезервирован
//! под будущие стадии (long_path / pipe_reader / url_guard / ipc_limits). Чтобы не
//! загромождать `cargo check` предупреждениями о «dead_code» до момента фактического
//! подключения, временно глушим предупреждения внутри подмодуля FluxCore на уровне
//! атрибута модуля. Как только код реально используется из основного конвейера,
//! атрибут будет снят (см. план launcher-fullstack-rehab).

#![allow(dead_code)]

pub mod arg_resolver;
pub mod asset_resolver;
pub mod auth_resolver;
pub mod chain_resolver;
pub mod conductor;
pub mod forge_resolver;
pub mod ipc_limits;
pub mod java_resolver;
pub mod launch_cache;
pub mod lib_resolver;
pub mod long_path;
pub mod native_resolver;
pub mod pipe_reader;
pub mod process_guard;
pub mod storage;
pub mod url_guard;
pub mod v3;
