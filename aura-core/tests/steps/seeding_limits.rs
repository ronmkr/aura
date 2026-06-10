use crate::AuraWorld;
use aura_core::TaskId;
use cucumber::{gherkin::Step, given, then, when};

#[then(
    expr = "the resolved configuration seeding ratio should be {float} and max_seeding_time should be {int} and stop_on_either should be {word}"
)]
fn then_assert_seeding_config(
    world: &mut AuraWorld,
    ratio: f32,
    time: u64,
    stop_on_either: String,
) {
    let config = world.resolved_config.as_ref().expect("No resolved config");
    assert_eq!(config.bittorrent.seeding.min_ratio, ratio);
    assert_eq!(config.bittorrent.seeding.max_seeding_time_secs, time);
    let stop_bool = stop_on_either.parse::<bool>().unwrap();
    assert_eq!(config.bittorrent.seeding.stop_on_either, stop_bool);
}

#[given(expr = "a running engine")]
async fn given_running_engine(world: &mut AuraWorld) {
    world.init_engine(|_cfg| {}).await;
}

#[when(expr = "I add a mock BitTorrent task with ID {int}")]
async fn when_add_mock_task(world: &mut AuraWorld, id: u64) {
    let engine = world.engine.as_ref().expect("Engine not running");

    engine
        .add_task_with_id(
            TaskId(id),
            format!("mock-task-{}", id),
            "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567".to_string(),
            aura_core::task::TaskType::BitTorrent,
        )
        .await
        .expect("Failed to add task");

    world.last_task_id = Some(TaskId(id));
}

#[then(expr = "the task {int} should have no seeding overrides")]
async fn then_task_has_no_overrides(world: &mut AuraWorld, id: u64) {
    let engine = world.engine.as_ref().expect("Engine not running");
    let active = engine
        .tell_active()
        .await
        .expect("Failed to get active tasks");
    let task = active
        .iter()
        .find(|t| t.id == TaskId(id))
        .expect("Task not found");
    assert_eq!(task.seed_ratio(), None);
    assert_eq!(task.seed_time(), None);
}

#[when(expr = "I change options for task {int} with:")]
async fn when_change_task_options(world: &mut AuraWorld, id: u64, step: &Step) {
    let engine = world.engine.as_ref().expect("Engine not running");
    let table = step.table.as_ref().expect("Expected a table");

    let mut seed_ratio = None;
    let mut seed_time = None;

    for row in table.rows.iter().skip(1) {
        let option = &row[0];
        let value = &row[1];
        match option.as_str() {
            "seed-ratio" => seed_ratio = Some(value.parse::<f32>().unwrap()),
            "seed-time" => seed_time = Some(value.parse::<u32>().unwrap()),
            _ => panic!("Unknown option: {}", option),
        }
    }

    engine
        .change_option(TaskId(id), None, None, seed_ratio, seed_time)
        .await
        .expect("Failed to change options");

    // Give the actor thread a moment to process the command
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

#[then(expr = "the task {int} should have seed_ratio {float} and seed_time {int}")]
async fn then_assert_task_overrides(world: &mut AuraWorld, id: u64, ratio: f32, time: u32) {
    let engine = world.engine.as_ref().expect("Engine not running");
    let active = engine
        .tell_active()
        .await
        .expect("Failed to get active tasks");
    let task = active
        .iter()
        .find(|t| t.id == TaskId(id))
        .expect("Task not found");
    assert_eq!(task.seed_ratio(), Some(ratio));
    assert_eq!(task.seed_time(), Some(time));
}
