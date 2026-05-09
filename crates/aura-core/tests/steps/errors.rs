use crate::AuraWorld;
use cucumber::{given, then, when};

#[given(expr = "an HTTP mirror that is returning {string}")]
async fn given_http_mirror_returning_error(_world: &mut AuraWorld, _error: String) {}

#[when(expr = "the {string} receives the error")]
async fn when_worker_receives_error(_world: &mut AuraWorld, _worker: String) {}

#[then(expr = "it should wait {int} seconds before the first retry")]
async fn then_wait_first_retry(_world: &mut AuraWorld, _secs: u32) {}

#[then(expr = "it should wait {int} seconds before the second retry")]
async fn then_wait_second_retry(_world: &mut AuraWorld, _secs: u32) {}

#[then(expr = "it should mark the source as {string} after {int} attempts")]
async fn then_mark_source_after_attempts(_world: &mut AuraWorld, _state: String, _attempts: u32) {}

#[given(expr = "a Metalink task with HTTP Mirror A and FTP Mirror B")]
async fn given_metalink_task_mirrors(_world: &mut AuraWorld) {}

#[when(expr = "Mirror A returns a {string}")]
async fn when_mirror_a_returns_error(_world: &mut AuraWorld, _error: String) {}

#[then(expr = "the {string} should automatically switch all pending ranges to Mirror B")]
async fn then_switch_pending_ranges(_world: &mut AuraWorld, _actor: String) {}

#[then(expr = "Mirror A should be marked as {string} in the task metadata")]
async fn then_mirror_a_marked(_world: &mut AuraWorld, _state: String) {}

#[given(expr = "the destination drive has only {int} MB of free space")]
async fn given_drive_free_space(_world: &mut AuraWorld, _mb: u32) {}

#[when(expr = "I add a task for a {int} MB file")]
async fn when_add_large_task(_world: &mut AuraWorld, _mb: u32) {}

#[then(expr = "the {string} should fail the pre-allocation")]
async fn then_fail_preallocation(_world: &mut AuraWorld, _engine: String) {}

#[then(expr = "the {string} should immediately pause the task with {string}")]
async fn then_pause_task_with_error(_world: &mut AuraWorld, _actor: String, _error: String) {}

#[then(expr = "the .aura control file should be preserved to allow resumption after cleanup")]
async fn then_preserve_aura_file(_world: &mut AuraWorld) {}
