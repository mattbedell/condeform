use std::collections::HashSet;
use std::env::current_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use etcetera::app_strategy::{AppStrategy, AppStrategyArgs, Xdg};
use serde::{Deserialize, Serialize};

const REGIONS: [&'static str; 3] = ["us-east-1", "eu-central-1", "ap-northeast-1"];
const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const APP_NAME: &str = env!("CARGO_PKG_NAME");

mod cli;

#[derive(Deserialize, Serialize)]
struct Config {
    environment: String,
    region: String,
    module: String,
    infra_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            environment: "gp-nonprod".to_string(),
            region: "us-east-1".to_string(),
            module: "vpc".to_string(),
            infra_dir: "../../".to_string(),
        }
    }
}

fn main() {
    let strategy = Xdg::new(AppStrategyArgs {
        top_level_domain: "org".to_string(),
        author: AUTHORS.to_string(),
        app_name: APP_NAME.to_string(),
    })
    .unwrap();

    let state_dir = strategy.state_dir().unwrap();

    fs::create_dir_all(&state_dir).expect("Could not create state directory");
    let state_path = get_repo_state_filepath(&state_dir);

    let previous_state = fs::read_to_string(&state_path);
    let cur_dir = current_dir().unwrap();
    let state = match previous_state {
        Ok(str) => toml::from_str(&str).unwrap(),
        Err(_) => {
            let default_state = Config {
                module: cur_dir.file_name().unwrap().to_str().unwrap().to_string(),
                ..Config::default()
            };
            write_state(&state_path, &default_state);
            default_state
        }
    };

    let cli = cli::Cli::parse();

    use cli::Commands::*;
    match &cli.command {
        Init { interactive } => {
            let config = {
                if let Some(true) = interactive {
                    let state = get_config_with_input(&state, &cur_dir);
                    write_state(&state_path, &state);
                    state
                } else {
                    state
                }
            };

            let module_path = get_module_var_dir(&config, "backend");

            let args = vec![
                "init",
                "-get=true",
                "-force-copy",
                "-backend-config",
                module_path.to_str().unwrap(),
                "-reconfigure",
            ];

            println!("terraform {}", args.join(" "));

            Command::new("terraform")
                .args(args)
                .status()
                .expect("failed to start terraform");
        }
        Edit => {
            let new_state = get_config_with_input(&state, &cur_dir);
            write_state(&state_path, &new_state);
        }
        Plan => {
            let module_path = get_module_var_dir(&state, "terraform");
            let args = vec![
                "plan",
                "-var-file",
                module_path.to_str().unwrap(),
                "-out=./plan.plan",
                "-lock-timeout=30s",
            ];

            println!("terraform {}", args.join(" "));

            Command::new("terraform")
                .args(args)
                .status()
                .expect("failed to start terraform");
        }
        Destroy => {
            let module_path = get_module_var_dir(&state, "terraform");
            let args = vec!["destroy", "-var-file", module_path.to_str().unwrap()];

            println!("terraform {}", args.join(" "));

            Command::new("terraform")
                .args(args)
                .status()
                .expect("failed to start terraform");
        }
    }
}

fn region_input(regions: Vec<&str>, theme: &ColorfulTheme) -> String {
    let region_index = Select::with_theme(theme)
        .with_prompt("Select region or <ESC> for text input")
        .items(&regions)
        .default(0)
        .interact_opt()
        .expect("Exited");

    match region_index {
        Some(idx) => regions[idx].to_owned(),
        None => {
            let default_region = regions[0].to_owned();
            Input::<String>::with_theme(theme)
                .with_prompt("Region")
                .default(default_region)
                .interact_text()
                .expect("Cannot process input")
        }
    }
}

fn get_git_root() -> PathBuf {
    let repo_root = Command::new("git")
        .args(vec!["rev-parse", "--show-toplevel"])
        .output()
        .expect("Could not determine git repo");
    let mut git_path: String = String::from_utf8(repo_root.stdout).unwrap();
    git_path = git_path
        .strip_suffix("\n")
        .map_or(git_path.to_owned(), |v| v.to_string());

    let mut path = PathBuf::new();
    path.push(&git_path);
    path
}

fn get_repo_state_filepath(state_dir: &PathBuf) -> PathBuf {
    let git_root = get_git_root();

    let filename = git_root.to_str().unwrap().to_string().replace("/", "%");

    let mut state_filepath = Path::new(&state_dir).to_path_buf();
    state_filepath.push(filename);
    state_filepath.set_extension("toml");
    state_filepath
}

fn get_module_var_dir(config: &Config, basename: &str) -> PathBuf {
    let mut module_path = PathBuf::new();
    module_path.push(&config.infra_dir);
    module_path.push(&config.environment);
    module_path.push(&config.region);
    module_path.push(&config.module);

    module_path.push(basename);
    module_path.set_extension("tfvars");
    module_path
}

fn get_config_with_input(state: &Config, cwd: &PathBuf) -> Config {
    let theme = ColorfulTheme::default();
    let mut regions = REGIONS.to_vec();
    let mut uniq = HashSet::new();
    regions.sort_unstable();
    regions.insert(0, &state.region);
    regions.retain(|v| uniq.insert(*v));

    let environment = Input::<String>::with_theme(&theme)
        .with_prompt("Environment")
        .default(state.environment.to_string())
        .interact_text()
        .expect("Cannot process input");
    let region = region_input(regions, &theme);
    let module = Input::<String>::with_theme(&theme)
        .with_prompt("Module")
        .with_initial_text(current_dir().map_or(state.module.to_string(), |v| {
            v.file_name().unwrap().to_str().unwrap().to_string()
        }))
        .default(state.module.to_string())
        .interact_text()
        .expect("Cannot process input");

    let infra_dir = Input::<String>::with_theme(&theme)
        .with_prompt("Infra Dir")
        .default(state.infra_dir.to_string())
        .interact_text()
        .expect("Cannot process input");

    let infra_path = cwd.join(infra_dir).canonicalize().unwrap();

    Config {
        environment,
        region,
        module,
        infra_dir: infra_path.to_str().unwrap().to_string(),
    }
}

fn write_state(state_path: &PathBuf, config: &Config) -> () {
    fs::write(state_path, toml::to_string(config).unwrap()).expect("Could not write state file");
}
