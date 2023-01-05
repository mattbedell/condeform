use anyhow;
use std::collections::HashSet;
use std::env::current_dir;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use dialoguer::{theme::ColorfulTheme, Input, Select};
use etcetera::app_strategy::{AppStrategy, AppStrategyArgs, Xdg};
use serde::{Deserialize, Serialize};

const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const APP_NAME: &str = env!("CARGO_PKG_NAME");

mod cli;
mod error;

use error::ModuleError;

#[derive(Deserialize, Serialize)]
struct Config {
    environment: Option<String>,
    region: String,
    module: String,
    infra_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            environment: None,
            region: "us-east-1".to_string(),
            module: "vpc".to_string(),
            infra_dir: "../../".to_string(),
        }
    }
}

fn main() -> Result<(), anyhow::Error> {
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
            write_state(&state_path, &default_state)?;
            default_state
        }
    };

    let cli = cli::Cli::parse();

    use cli::Commands::*;
    match &cli.command {
        Init { interactive } => {
            let config = {
                if let Some(true) = interactive {
                    let state = get_config_with_input(&state, &cur_dir)?;
                    write_state(&state_path, &state)?;
                    state
                } else {
                    state
                }
            };

            let module_path = get_module_var_dir(&config, "backend")?;

            let args = vec![
                "init",
                "-get=true",
                "-force-copy",
                "-backend-config",
                module_path.to_str().unwrap(),
                "-reconfigure",
            ];

            println!("terraform {}", args.join(" "));

            Command::new("terraform").args(args).status()?;
        }
        Edit => {
            let new_state = get_config_with_input(&state, &cur_dir)?;
            write_state(&state_path, &new_state)?;
        }
        Plan => {
            let module_path = get_module_var_dir(&state, "terraform")?;
            let args = vec![
                "plan",
                "-var-file",
                module_path.to_str().unwrap(),
                "-out=./plan.plan",
                "-lock-timeout=30s",
            ];

            println!("terraform {}", args.join(" "));

            Command::new("terraform").args(args).status()?;
        }
        Destroy => {
            let module_path = get_module_var_dir(&state, "terraform")?;
            let args = vec!["destroy", "-var-file", module_path.to_str().unwrap()];

            println!("terraform {}", args.join(" "));

            Command::new("terraform").args(args).status()?;
        }
    };
    Ok(())
}

fn env_input(
    infra_dir: &String,
    config: &Config,
    theme: &ColorfulTheme,
) -> anyhow::Result<String> {
    let infra_path = Path::new(infra_dir).to_path_buf();

    let mut uniq = HashSet::new();

    let mut items: Vec<String> = get_dirnames_from_path(&infra_path)
        .filter(|v| v != "terraform")
        .collect();

    items.sort_unstable();

    if let Some(env) = &config.environment {
        items.insert(0, env.to_owned());
    }

    items.retain(|v| uniq.insert(v.to_owned()));

    let env_index = Select::with_theme(theme)
        .with_prompt("Environment")
        .items(&items)
        .default(0)
        .interact_opt()
        .expect("Cannot process input");

    if let Some(idx) = env_index {
        Ok(items[idx].to_owned())
    } else {
        Err(ModuleError::IncompleteConfig("environment".to_string()).into())
    }
}

fn get_dirnames_from_path(path: &PathBuf) -> impl Iterator<Item=String> {
    path.read_dir()
        .unwrap()
        .filter_map(|v| v.ok())
        .map(|v| v.path())
        .filter(|v| v.is_dir())
        .filter_map(|v| {
            if let Some(filename) = v.file_name() {
                filename.to_str().and_then(|c| Some(c.to_string()))
            } else {
                None
            }
        })
}

fn region_input(config: &Config, infra_path: &PathBuf, env: &String, theme: &ColorfulTheme) -> String {

    let mut env_path = PathBuf::new();
    env_path = env_path.join(infra_path);
    env_path.push(env);

    let mut items: Vec<String> = get_dirnames_from_path(&env_path)
        .collect();


    let mut uniq = HashSet::new();
    items.sort_unstable();

    items.insert(0, config.region.to_owned());
    items.retain(|v| uniq.insert(v.to_owned()));
    let region_index = Select::with_theme(theme)
        .with_prompt("Select region or <ESC> for text input")
        .items(&items)
        .default(0)
        .interact_opt()
        .expect("Exited");

    match region_index {
        Some(idx) => items[idx].to_owned(),
        None => {
            let default_region = items[0].to_owned();
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

fn get_module_var_dir(config: &Config, basename: &str) -> Result<PathBuf, ModuleError> {
    let mut module_path = PathBuf::new();
    module_path.push(&config.infra_dir);
    if let Some(env) = &config.environment {
        module_path.push(env);
    }
    module_path.push(&config.region);
    module_path.push(&config.module);

    if let false = module_path.is_dir() {
        return Err(ModuleError::NotADirectory {
            environment: config.environment.as_ref().unwrap().to_owned(),
            region: config.region.to_owned(),
        });
    }

    module_path.push(basename);
    module_path.set_extension("tfvars");
    Ok(module_path)
}

fn get_config_with_input(state: &Config, cwd: &PathBuf) -> anyhow::Result<Config> {
    let theme = ColorfulTheme::default();

    let infra_dir = Input::<String>::with_theme(&theme)
        .with_prompt("Infra Dir")
        .default(state.infra_dir.to_string())
        .interact_text()
        .expect("Cannot process input");

    let infra_path = cwd.join(&infra_dir).canonicalize().unwrap();

    let environment = env_input(&infra_dir, state, &theme)?;
    let region = region_input(&state, &infra_path, &environment, &theme);
    let module = Input::<String>::with_theme(&theme)
        .with_prompt("Module")
        .with_initial_text(current_dir().map_or(state.module.to_string(), |v| {
            v.file_name().unwrap().to_str().unwrap().to_string()
        }))
        .default(state.module.to_string())
        .interact_text()
        .expect("Cannot process input");


    Ok(Config {
        environment: Some(environment),
        region,
        module,
        infra_dir: infra_path.to_str().unwrap().to_string(),
    })
}

fn write_state(state_path: &PathBuf, config: &Config) -> anyhow::Result<()> {
    fs::write(state_path, toml::to_string(config).unwrap())?;
    Ok(())
}
