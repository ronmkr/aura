use crate::Config;
use cucumber::{gherkin::Step, given, then, when};
use std::fs::{self, File};
use std::io::Write;

#[given(expr = "a mock environment with user config file at {string} containing:")]
fn given_user_config(world: &mut crate::AuraWorld, _path: String, step: &Step) {
    if world.original_home.is_none() {
        world.original_home = std::env::var_os("HOME");
    }
    let content = step.docstring.as_ref().expect("docstring not found");
    let home_path = world.temp_dir.path().to_path_buf();
    std::env::set_var("HOME", &home_path);

    let config_dir = home_path.join(".config").join("aura");
    fs::create_dir_all(&config_dir).unwrap();
    let config_file = config_dir.join("Aura.toml");
    let mut file = File::create(config_file).unwrap();
    file.write_all(content.as_bytes()).unwrap();
}

#[given(expr = "a local config file at {string} containing:")]
fn given_local_config(world: &mut crate::AuraWorld, _path: String, step: &Step) {
    if world.original_cwd.is_none() {
        world.original_cwd = std::env::current_dir().ok();
    }
    let content = step.docstring.as_ref().expect("docstring not found");
    let temp_dir = world.temp_dir.path().to_path_buf();
    std::env::set_current_dir(&temp_dir).unwrap();

    let config_file = temp_dir.join("Aura.toml");
    let mut file = File::create(config_file).unwrap();
    file.write_all(content.as_bytes()).unwrap();
}

#[when(expr = "I resolve the configuration with custom config path {string}")]
fn when_resolve_config(world: &mut crate::AuraWorld, custom_path: String) {
    let path_opt = if custom_path == "None" {
        None
    } else {
        Some(custom_path.as_str())
    };
    let config = Config::load_resolved(path_opt).expect("Failed to load resolved config");
    world.resolved_config = Some(config);
}

#[then(
    expr = "the resolved configuration should use local config port {int} and user config rpc_port {int}"
)]
fn then_assert_resolved_ports(world: &mut crate::AuraWorld, listen_port: u16, rpc_port: u16) {
    let config = world.resolved_config.as_ref().expect("No resolved config");
    assert_eq!(config.network.listen_port, listen_port);
    assert_eq!(config.network.rpc_port, rpc_port);
}

#[given(expr = "a configuration file loaded with:")]
fn given_config_loaded(world: &mut crate::AuraWorld, step: &Step) {
    let content = step.docstring.as_ref().expect("docstring not found");
    let config: Config = toml::from_str(content).expect("Failed to parse TOML");
    world.resolved_config = Some(config);
}

#[when(expr = "I apply CLI overrides:")]
fn when_apply_cli_overrides(world: &mut crate::AuraWorld, step: &Step) {
    let table = step.table.as_ref().expect("Expected a table");
    let mut download_dir = None;
    let mut limit = None;

    for row in table.rows.iter().skip(1) {
        let option = &row[0];
        let value = &row[1];
        match option.as_str() {
            "download_dir" => download_dir = Some(value.clone()),
            "limit" => limit = Some(value.parse::<u64>().unwrap()),
            _ => panic!("Unknown option: {}", option),
        }
    }

    let config = world.resolved_config.as_mut().expect("No resolved config");
    config.apply_cli_overrides(download_dir, limit, None, None, None, None, None);
}

#[then(
    expr = "the final configuration should use download_dir {string} and global_download_limit {int}"
)]
fn then_assert_overridden_config(world: &mut crate::AuraWorld, download_dir: String, limit: u64) {
    let config = world.resolved_config.as_ref().expect("No resolved config");
    assert_eq!(config.storage.download_dir, download_dir);
    assert_eq!(config.bandwidth.global_download_limit, limit);
}
