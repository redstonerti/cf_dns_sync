use chrono::*;
use colored::Colorize;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, MultiSelect};
use home::home_dir;
use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;
use std::fs::{self, File, OpenOptions};
use std::io::prelude::*;
use std::path::PathBuf;
use std::process;
use std::{io::Read, net::IpAddr, path::Path};
use sysinfo::{System, SystemExt};

#[derive(Deserialize, Debug)]
pub struct IncompleteConfig {
    #[serde(default = "default_stwpr")]
    seconds_to_wait_per_restart: u32,
    authentication: Option<AuthenticationConfig>,
    #[serde(default = "default_log_config")]
    log_config: LogConfig,
    #[serde(default = "default_dns_config")]
    dns_config: Vec<DNSRecord>,
}
fn default_stwpr() -> u32 {
    300
}
fn default_log_config() -> LogConfig {
    LogConfig::default()
}
fn default_dns_config() -> Vec<DNSRecord> {
    vec![]
}
#[derive(Deserialize, Debug, Serialize, Clone)]
pub struct Config {
    pub seconds_to_wait_per_restart: u32,
    pub authentication: AuthenticationConfig,
    pub log_config: LogConfig,
    pub dns_config: Vec<DNSRecord>,
}
#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct AuthenticationConfig {
    pub email: String,
    pub api_key: String,
    pub zone_id: String,
}
#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct LogConfig {
    #[serde(default = "get_log_folder")]
    pub log_folder_path: String,
    #[serde(default = "default_slbs")]
    pub separate_logs_by_session: bool,
    pub session_number: Option<i32>,
    #[serde(default = "default_display_config")]
    pub display: DisplayConfig,
    #[serde(default = "default_show_config")]
    pub show: ShowConfig,
}
impl Default for LogConfig {
    fn default() -> Self {
        LogConfig {
            log_folder_path: get_log_folder(),
            separate_logs_by_session: default_slbs(),
            session_number: Some(1),
            display: DisplayConfig::default(),
            show: ShowConfig::default(),
        }
    }
}
fn default_slbs() -> bool {
    true
}
fn default_display_config() -> DisplayConfig {
    DisplayConfig::default()
}
#[derive(Deserialize, Debug, Clone, Copy, Serialize)]
pub struct DisplayConfig {
    #[serde(default = "default_display")]
    pub date: bool,
    #[serde(default = "default_display")]
    pub time: bool,
    #[serde(default = "default_display")]
    pub log_type: bool,
}
impl Default for DisplayConfig {
    fn default() -> Self {
        DisplayConfig {
            date: true,
            time: true,
            log_type: true,
        }
    }
}
fn default_display() -> bool {
    true
}
fn default_show_config() -> ShowConfig {
    ShowConfig::default()
}
#[derive(Deserialize, Debug, Clone, Copy, Serialize)]
pub struct ShowConfig {
    #[serde(default = "default_show")]
    pub logs: bool,
    #[serde(default = "default_show")]
    pub warnings: bool,
    #[serde(default = "default_show")]
    pub errors: bool,
}
impl Default for ShowConfig {
    fn default() -> Self {
        ShowConfig {
            logs: true,
            warnings: true,
            errors: true,
        }
    }
}
fn default_show() -> bool {
    true
}
#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct DNSRecord {
    pub record_type: String,
    pub name: String,
    pub content: String,
    pub proxy_status: Option<bool>,
    pub ttl: i32,
    pub id: String,
    pub sync: Option<bool>,
}
impl Config {
    fn default() -> Result<Self, ()> {
        Ok(Config {
            seconds_to_wait_per_restart: 300,
            authentication: match AuthenticationConfig::default() {
                Ok(authentication) => authentication,
                Err(()) => return Err(()),
            },
            log_config: LogConfig::default(),
            dns_config: vec![],
        })
    }
    pub fn save_to_json(&self, path: &Path) -> Result<(), ()> {
        if path.exists() {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(err) => panic!("Failed to delete old config file{}", format_err(err)),
            }
        }
        let string = match serde_json::to_string(&self) {
            Ok(string) => string,
            Err(err) => panic!(
                "Failed to convert new config file to string{}",
                format_err(err)
            ),
        };
        let string = jsonformat::format(&string, jsonformat::Indentation::Tab);
        let string = string.replace("\\/", "/");
        match write_to_file(path, string, None) {
            Ok(()) => Ok(()),
            Err(()) => Err(()),
        }
    }
    fn to_incomplete(&self) -> IncompleteConfig {
        IncompleteConfig {
            seconds_to_wait_per_restart: self.seconds_to_wait_per_restart,
            authentication: Some(self.authentication.clone()),
            log_config: self.log_config.clone(),
            dns_config: self.dns_config.clone(),
        }
    }
}
impl AuthenticationConfig {
    fn default() -> Result<Self, ()> {
        let email: String = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Your email")
            .validate_with({
                let mut force = None;
                move |input: &String| -> Result<(), &str> {
                    if input.contains('@') || force.as_ref().map_or(false, |old| old == input) {
                        Ok(())
                    } else {
                        force = Some(input.clone());
                        Err("This is not a mail address; type the same value again to force use")
                    }
                }
            })
            .interact_text()
        {
            Ok(email) => email,
            Err(err) => panic!("Couldn't get email{}", format_err(err)),
        };
        let zone_id: String = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Your zone id")
            .interact_text()
        {
            Ok(zone_id) => zone_id,
            Err(err) => panic!("Couldn't get zone id{}", format_err(err)),
        };
        let api_key: String = match Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Your API Key")
            .interact_text()
        {
            Ok(api_key) => api_key,
            Err(err) => panic!("Couldn't get API Key{}", format_err(err)),
        };
        Ok(AuthenticationConfig {
            email,
            api_key,
            zone_id,
        })
    }
}
impl IncompleteConfig {
    fn is_complete(&mut self) -> bool {
        let mut is_complete = true;
        match self.authentication {
            None => is_complete = false,
            _ => {}
        }
        is_complete
    }
    fn complete(&mut self) -> Result<Config, ()> {
        if !self.is_complete() {
            if !is_terminal() {
                println!("Couldn't setup config because process is not running in a terminal. Please configure manually before running.");
                process::exit(0);
            }
        }
        let authentication = match self.authentication.clone() {
            Some(authentication_config) => authentication_config.clone(),
            None => match AuthenticationConfig::default() {
                Ok(authentication) => authentication,
                Err(()) => return Err(()),
            },
        };
        let config = Config {
            seconds_to_wait_per_restart: self.seconds_to_wait_per_restart,
            authentication,
            log_config: self.log_config.clone(),
            dns_config: self.dns_config.clone(),
        };
        Ok(config)
    }
    fn _reconfigure(&mut self) {}
}
enum CustomError {
    ConvertIntoString,
    UnsuccessfullCloudflareRequest(String),
    UReqRequstFailed(ureq::Error),
}
#[derive(Clone, Copy)]
pub enum LogType {
    Log,
    Warn,
    Error,
}
#[tokio::main]
pub async fn process() {
    check_for_root();
    let (config, _) = get_config();
    let mut wait_on_startup = true;
    loop {
        if wait_on_startup {
            wait_on_startup = false;
        } else {
            log_to_file_and_console(
                &format!(
                    "Waiting {} seconds to restart...",
                    config.seconds_to_wait_per_restart
                ),
                LogType::Log,
                &config.log_config,
            );
            std::thread::sleep(std::time::Duration::from_secs_f32(
                config.seconds_to_wait_per_restart as f32,
            ));
        }
        let ip = match public_ip::addr().await {
            Some(_ip_addr) => {
                log_to_file_and_console(
                    "Successfully obtained public ip address",
                    LogType::Log,
                    &config.log_config,
                );
                _ip_addr
            }
            None => {
                log_to_file_and_console(
                    "Couldn't get public ip address",
                    LogType::Error,
                    &config.log_config,
                );
                log_to_file_and_console("Retrying...", LogType::Error, &config.log_config);
                continue;
            }
        };
        let mut failures = false;
        let mut records_changed_successfully = 0;
        for record in config.dns_config.iter() {
            if let Some(true) = record.sync {
                match set_ip(
                    &ip,
                    &record.name,
                    &record.id,
                    &config.authentication,
                    &config.log_config,
                ) {
                    Ok(()) => {
                        log_to_file_and_console(
                            &format!("Successfully set ip for {}", &record.name),
                            LogType::Log,
                            &config.log_config,
                        );
                        records_changed_successfully += 1;
                    }
                    Err(err) => match err {
                        CustomError::ConvertIntoString => {
                            log_to_file_and_console(
                                "Failed to convert cloudflare's result into a string, retrying...",
                                LogType::Warn,
                                &config.log_config,
                            );
                            failures = true;
                            continue;
                        }
                        CustomError::UnsuccessfullCloudflareRequest(string) => {
                            log_to_file_and_console(
                                &format!("The cloudflare request was unsuccessful. Here's the result:\n{string}"),
                                LogType::Warn,
                                &config.log_config,
                            );
                            failures = true;
                            continue;
                        }
                        CustomError::UReqRequstFailed(err) => {
                            log_to_file_and_console(
                                &format!("The ureq request failed{}", format_err(err)),
                                LogType::Error,
                                &config.log_config,
                            );
                            failures = true;
                            log_to_file_and_console(
                                "Retrying...",
                                LogType::Error,
                                &config.log_config,
                            );
                            continue;
                        }
                    },
                }
            }
        }
        if failures {
            if records_changed_successfully > 0 {
                log_to_file_and_console(
                    &format!(
                        "Only {} out of {} records were changed successfully",
                        records_changed_successfully,
                        config.dns_config.len()
                    ),
                    LogType::Warn,
                    &config.log_config,
                );
            } else {
                log_to_file_and_console(
                    "All record changes failed",
                    LogType::Warn,
                    &config.log_config,
                );
            }
        } else {
            if records_changed_successfully > 0 {
                log_to_file_and_console(
                    "All records changed successfully!",
                    LogType::Log,
                    &config.log_config,
                );
            } else {
                log_to_file_and_console(
                    "No records were changed",
                    LogType::Log,
                    &config.log_config,
                );
            }
        }
    }
}
pub fn get_config() -> (Config, PathBuf) {
    let (mut incomplete_config, config_path, config_file_contents) = match get_incomplete_config() {
        Ok((incomplete_config, config_path, config_file_contents)) => {
            (incomplete_config, config_path, Some(config_file_contents))
        }
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {
                if is_terminal() {
                    let config = match Config::default() {
                        Ok(config) => config,
                        Err(()) => panic!("Failed to get config"),
                    };
                    let config_path = match get_config_path() {
                        Ok(config_path) => config_path,
                        Err(()) => panic!("Failed to get config path"),
                    };
                    if let Err(()) = config.save_to_json(&config_path) {
                        log_to_console("Failed to save config", LogType::Error, &config.log_config);
                    }
                    (config.to_incomplete(), config_path, None)
                } else {
                    println!("There is no config file yet. You must create one using the configure command in a terminal.");
                    std::process::exit(0);
                }
            }
            _ => panic!("Failed to get config"),
        },
    };
    let mut config: Config;
    if !incomplete_config.is_complete() {
        config = match incomplete_config.complete() {
            Ok(config) => {
                match config.save_to_json(&config_path) {
                    Err(()) => {
                        log_to_console(
                            "Failed to save config file",
                            LogType::Error,
                            &config.log_config,
                        );
                    }
                    _ => {}
                };
                config
            }
            Err(()) => panic!("Failed to get config"),
        };
    } else {
        config = match incomplete_config.complete() {
            Ok(config) => {
                //Checks if the new config is any different to the one currently saved. If it is, it tries to save the new one.
                match serde_json::to_string(&config) {
                    Ok(new_config_file_contents) => {
                        let should_try_saving = match config_file_contents {
                            Some(config_file_contents) => {
                                config_file_contents != new_config_file_contents
                            }
                            None => true,
                        };
                        if should_try_saving {
                            match config.save_to_json(&config_path) {
                                Err(()) => {
                                    log_to_console(
                                        "Failed to save config file",
                                        LogType::Error,
                                        &config.log_config,
                                    );
                                }
                                _ => {}
                            };
                        }
                    }
                    Err(err) => {
                        log_to_console(
                            &format!(
                                "Failed to convert new config to string and save config file{}",
                                format_err(err)
                            ),
                            LogType::Error,
                            &config.log_config,
                        );
                    }
                };
                config
            }
            Err(()) => panic!("Failed to get config"),
        };
    }
    let previous_session_number = config.log_config.session_number.clone();
    config.log_config.session_number = match get_session_number(&config.log_config) {
        Some(session_number) => Some(session_number),
        None => {
            log_to_file_and_console(
                "Couldn't get session number",
                LogType::Warn,
                &config.log_config,
            );
            None
        }
    };
    if previous_session_number != config.log_config.session_number {
        if let Err(()) = config.save_to_json(&config_path) {
            log_to_console(
                "Failed to save config file after changing session number",
                LogType::Warn,
                &config.log_config,
            );
        }
    }
    log_to_file_and_console(
        "Attempting to retrieve DNS records",
        LogType::Log,
        &config.log_config,
    );
    update_dns_list(&mut config, &config_path);
    (config, config_path)
}
pub fn get_config_path() -> Result<PathBuf, ()> {
    let config_folder_path = match get_config_folder_path() {
        Ok(config_folder_path) => config_folder_path,
        Err(()) => return Err(()),
    };
    let config_path = config_folder_path.join("config.json");
    Ok(config_path)
}
pub fn get_config_folder_path() -> Result<PathBuf, ()> {
    let cargo_path = match home::cargo_home() {
        Ok(cargo_path) => cargo_path,
        Err(err) => panic!("Couldn't find cargo path{}", format_err(err)),
    };
    let user_path = match cargo_path.parent() {
        Some(path) => path,
        None => &cargo_path,
    };
    let config_path: PathBuf;
    if user_path == cargo_path {
        config_path = cargo_path.to_path_buf();
    } else {
        config_path = user_path.join(".config");
    }

    let folder_path = config_path.join("cf_dns_sync");
    if !folder_path.is_dir() {
        if let Err(_) = create_folder(&folder_path, None) {
            return Err(());
        }
    }
    Ok(folder_path)
}
pub fn get_log_folder() -> String {
    let config_folder_path = match get_config_folder_path() {
        Ok(config_folder_path) => config_folder_path,
        Err(()) => Path::new("./").into(),
    };
    match config_folder_path.to_str() {
        Some(string) => string.into(),
        None => "./".into(),
    }
}
pub fn get_incomplete_config() -> Result<(IncompleteConfig, PathBuf, String), std::io::Error> {
    let config_path = match get_config_path() {
        Ok(config_path) => config_path,
        Err(()) => panic!("Couldn't get config path"),
    };
    let incomplete_config: IncompleteConfig;
    if !config_path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File not found",
        ));
    }
    let config_file = match std::fs::File::open(config_path.clone()) {
        Ok(file) => file,
        Err(err) => return Err(err),
    };
    let mut buf_reader = std::io::BufReader::new(config_file);
    let mut config_file_contents = String::new();
    match buf_reader.read_to_string(&mut config_file_contents) {
        Err(err) => return Err(err),
        _ => {}
    };
    incomplete_config = match serde_json::from_str(&config_file_contents) {
        Ok(incomplete_config) => incomplete_config,
        Err(err) => {
            panic!(
                "It looks like your config.json is not formatted corectly. Here's the path to the config file: {}{}",
                format_err(&err),
                config_path.clone().to_str().unwrap()
            );
        }
    };
    Ok((incomplete_config, config_path, config_file_contents))
}
pub fn get_session_number(log_config: &LogConfig) -> Option<i32> {
    let folder_path = Path::new(&log_config.log_folder_path);
    let folder_path = folder_path.join("logs");
    if folder_path.is_dir() {
        if let Err(()) = create_folder(&folder_path, Some(&log_config)) {
            return None;
        }
        let log_paths = match fs::read_dir(folder_path) {
            Ok(paths) => paths,
            Err(err) => {
                log_to_console(
                    &format!("Couldn't read log folder{}", format_err(err)),
                    LogType::Error,
                    log_config,
                );
                return None;
            }
        };
        let log_paths_list: Vec<_> = log_paths.collect();
        if log_paths_list.len() == 0 {
            return Some(1);
        }
        let mut highest_value = None;
        for path in log_paths_list {
            let log_dir_entry = match path {
                Ok(path) => path,
                Err(err) => {
                    log_to_file_and_console(
                        &format!("Couldn't read log filename{}", format_err(err)),
                        LogType::Error,
                        log_config,
                    );
                    return None;
                }
            };
            let log_name = match log_dir_entry.file_name().into_string() {
                Ok(string) => string,
                Err(err) => log_to_console(
                    &format!(
                        "Couldn't convert file name OsString to string{}",
                        format_err(err)
                    ),
                    LogType::Warn,
                    log_config,
                ),
            };
            if log_name.len() < 12 {
                continue;
            }
            let session_indices: Vec<(usize, _)> = log_name.match_indices("session").collect();
            if session_indices.len() > 1 {
                continue;
            } else if session_indices.len() == 0 {
                continue;
            }
            if session_indices[0].0 != 0 {
                continue;
            }
            let log_number = *match &log_name[7..log_name.len() - 4].parse::<i32>() {
                Ok(num) => num,
                Err(_) => continue,
            };
            if Some(log_number) > highest_value {
                highest_value = Some(log_number);
            }
        }
        match highest_value {
            Some(highest_value) => {
                if log_config.separate_logs_by_session {
                    return Some(highest_value + 1);
                } else {
                    return Some(highest_value);
                }
            }
            None => return Some(1),
        }
    } else {
        return Some(1);
    }
}
pub fn log_to_console(string: &str, log_type: LogType, log_config: &LogConfig) -> String {
    let time_string = get_time(log_config.display.date, log_config.display.time);
    let mut log_string = "".to_string();
    if log_config.display.log_type {
        log_string = format!(
            "[{}] ",
            match log_type {
                LogType::Log => "LOG",
                LogType::Warn => "WARN",
                LogType::Error => "ERROR",
            }
        );
    }
    let string = format!("{time_string}{log_string}{string}");
    let should_print_colored: bool;
    if cfg!(windows) {
        match windows_major_version_number() {
            Ok(version_number) => {
                if version_number >= 11 {
                    should_print_colored = true;
                } else {
                    should_print_colored = false;
                }
            }
            Err(()) => should_print_colored = false,
        }
    } else {
        should_print_colored = true;
    }
    if should_print_colored {
        let colored_string = match log_type {
            LogType::Log => string.white(),
            LogType::Warn => string.yellow(),
            LogType::Error => string.red(),
        };
        println!("{colored_string}");
    } else {
        println!("{string}");
    }
    string
}
pub fn log_to_file_and_console(string: &str, log_type: LogType, log_config: &LogConfig) {
    match log_type {
        LogType::Log => {
            if !log_config.show.logs {
                return;
            }
        }
        LogType::Warn => {
            if !log_config.show.warnings {
                return;
            }
        }
        LogType::Error => {
            if !log_config.show.errors {
                return;
            }
        }
    }
    let string = log_to_console(string, log_type, log_config);
    let folder_path = Path::new(&log_config.log_folder_path);
    let folder_path = folder_path.join("logs");
    if !folder_path.is_dir() {
        if let Err(()) = create_folder(&folder_path, Some(log_config)) {
            return;
        }
    }
    let session_number = match log_config.session_number {
        Some(session_number) => session_number,
        None => {
            log_to_console(
                "Couldn't write to log file due to lack of session number",
                LogType::Warn,
                log_config,
            );
            return;
        }
    };
    let log_name = format!("session{}", session_number);
    let file_path = folder_path.join(format!("{log_name}.txt"));
    if let Err(_) = write_to_file(&file_path, string, Some(log_config)) {}
}
pub fn format_err(err: impl Debug) -> String {
    format!(". Here's the error:\n-------\n{:#?}", err)
}
pub fn update_dns_list(config: &mut Config, config_path: &PathBuf) {
    loop {
        //Get DNS record list
        let result = match get_dns_record_list(&config) {
            Ok(result) => result,
            Err(()) => continue,
        };
        //Convert response to json
        let json: Value = match serde_json::from_str(&result) {
            Ok(value) => value,
            Err(err) => {
                log_to_file_and_console(
                    &format!(
                        "Converting the cloudflare result to json failed{}",
                        format_err(err)
                    ),
                    LogType::Error,
                    &config.log_config,
                );
                continue;
            }
        };
        //Convert json to value
        let result = match json.get("result") {
            Some(result) => result,
            None => {
                log_to_file_and_console(
                    "Getting result from response failed",
                    LogType::Error,
                    &config.log_config,
                );
                continue;
            }
        };
        let array = match result.as_array() {
            Some(array) => array,
            None => {
                log_to_file_and_console(
                    "Converting result to array failed",
                    LogType::Error,
                    &config.log_config,
                );
                continue;
            }
        };
        let mut new_dns_records: Vec<DNSRecord> = vec![];
        for val in array {
            let dns_record = match convert_val_to_dns_record(val, &config) {
                Ok(dns_record) => dns_record,
                Err(()) => continue,
            };
            match dns_record {
                Some(dns_record) => new_dns_records.push(dns_record),
                _ => {}
            };
        }
        let mut new_record_references: Vec<usize> = vec![];
        for i in 0..new_dns_records.len() {
            let mut exists = false;
            for record2 in config.dns_config.iter() {
                if new_dns_records[i].id == record2.id {
                    if let Some(sync) = record2.sync {
                        new_dns_records[i].sync = Some(sync);
                        exists = true;
                    }
                }
            }
            if !exists {
                new_record_references.push(i);
            }
        }

        //Ask the user whether or not the new records should be synced if running in terminal
        if new_record_references.len() > 0 && is_terminal() {
            let mut records: Vec<DNSRecord> = vec![];
            for i in new_record_references.iter() {
                records.push(new_dns_records[*i].clone());
            }
            let ((multiselected, ids), defaults) = create_selection_list(&records);
            let selections = match MultiSelect::with_theme(&ColorfulTheme::default())
                .with_prompt("Select which new records need to be synced")
                .items(&multiselected[..])
                .defaults(&defaults[..])
                .interact()
            {
                Ok(list) => list,
                Err(err) => {
                    log_to_file_and_console(
                        &format!("Failed to select records{}", format_err(err)),
                        LogType::Error,
                        &config.log_config,
                    );
                    panic!("Failed to select records");
                }
            };
            for selection in selections {
                for i in new_record_references.iter() {
                    if new_dns_records[*i].id == ids[selection] {
                        new_dns_records[*i].sync = Some(true);
                    } else if new_dns_records[*i].sync == None {
                        new_dns_records[*i].sync = Some(false);
                    }
                }
            }
        }
        config.dns_config = new_dns_records;
        //Save new dns list
        match config.save_to_json(&config_path) {
            Ok(()) => log_to_file_and_console(
                "Saved config successfully",
                LogType::Log,
                &config.log_config,
            ),
            Err(()) => {
                log_to_file_and_console("Failed to save config", LogType::Warn, &config.log_config)
            }
        }
        break;
    }
}
pub fn create_selection_list(records: &Vec<DNSRecord>) -> ((Vec<String>, Vec<String>), Vec<bool>) {
    let mut multiselected: Vec<String> = vec![];
    let mut ids: Vec<String> = vec![];
    let mut longest = 0;
    for record in records {
        if record.name.len() > longest {
            longest = record.name.len();
        }
    }
    let mut defaults: Vec<bool> = vec![];
    for record in records {
        let name = record.name.clone();
        let content = record.content.clone();
        let proxy_status = match record.proxy_status.clone() {
            Some(proxy_status) => match proxy_status {
                true => "true",
                false => "false",
            },
            None => "Unknown",
        };
        let id = record.id.clone();
        let ttl = record.ttl.clone();
        multiselected.push(format!(
            "{:7} {:width$} {:10} {:17} {:15} {:8} {:6} {}",
            "Name",
            name,
            "Content",
            content,
            "Proxy Status",
            proxy_status,
            "TTL",
            ttl,
            width = longest + 3
        ));
        ids.push(id);
        defaults.push(false);
    }
    ((multiselected, ids), defaults)
}
fn get_dns_record_list(config: &Config) -> Result<String, ()> {
    match ureq::get(&format!(
        "https://api.cloudflare.com/client/v4/zones/{}/dns_records",
        &config.authentication.zone_id
    ))
    .set("X-Auth-Email", &config.authentication.email)
    .set("X-Auth-Key", &config.authentication.api_key)
    .set("Content-Type", "application/json")
    .call()
    {
        Ok(result) => {
            let result_string = match result.into_string() {
                Ok(string) => string,
                Err(err) => {
                    log_to_file_and_console(
                        &format!("Failed to turn result into string{}", format_err(err)),
                        LogType::Error,
                        &config.log_config,
                    );
                    log_to_file_and_console("Retrying...", LogType::Error, &config.log_config);
                    return Err(());
                }
            };
            let formatted_result_string =
                jsonformat::format(&result_string, jsonformat::Indentation::Tab);
            match formatted_result_string.find("\"success\": true") {
                Some(_) => log_to_file_and_console(
                    "Successfully obtained DNS records",
                    LogType::Log,
                    &config.log_config,
                ),
                None => {
                    log_to_file_and_console(
                        &format!(
                            "The cloudflare request was unsuccessful. Here's the result: {}",
                            formatted_result_string
                        ),
                        LogType::Warn,
                        &config.log_config,
                    );
                    return Err(());
                }
            }
            Ok(result_string)
        }
        Err(err) => {
            log_to_file_and_console(
                &format!("Couldn't send the list DNS request{}", format_err(err)),
                LogType::Error,
                &config.log_config,
            );
            log_to_file_and_console("Retrying...", LogType::Error, &config.log_config);
            return Err(());
        }
    }
}
fn convert_val_to_dns_record(val: &Value, config: &Config) -> Result<Option<DNSRecord>, ()> {
    Ok(Some(DNSRecord {
        name: match val.get("name") {
            Some(name) => name.to_string().replace("\"", ""),
            None => {
                log_to_file_and_console(
                    "Getting name from dns record failed",
                    LogType::Error,
                    &config.log_config,
                );
                return Err(());
            }
        },
        id: match val.get("id") {
            Some(id) => id.to_string().replace("\"", ""),
            None => {
                log_to_file_and_console(
                    "Getting id from dns record failed",
                    LogType::Error,
                    &config.log_config,
                );
                return Err(());
            }
        },
        sync: None,
        record_type: match val.get("type") {
            Some(name) => {
                let record_type = name.to_string().replace("\"", "");
                match record_type.as_str() {
                    "A" => record_type,
                    _ => return Ok(None),
                }
            }
            None => {
                log_to_file_and_console(
                    "Getting type from dns record failed",
                    LogType::Error,
                    &config.log_config,
                );
                return Err(());
            }
        },
        content: match val.get("content") {
            Some(name) => name.to_string().replace("\"", ""),
            None => {
                log_to_file_and_console(
                    "Getting content from dns record failed",
                    LogType::Error,
                    &config.log_config,
                );
                return Err(());
            }
        },
        proxy_status: match val.get("proxied") {
            Some(proxied) => {
                let proxy_string = proxied.to_string().replace("\"", "");
                let proxy_str = proxy_string.as_str();
                let status = match proxy_str {
                    "true" => Some(true),
                    "false" => Some(false),
                    _ => None,
                };
                status
            }
            None => {
                log_to_file_and_console(
                    "Getting proxy status from dns record failed",
                    LogType::Error,
                    &config.log_config,
                );
                return Err(());
            }
        },
        ttl: match val.get("ttl") {
            Some(ttl_str) => {
                let ttl_str = ttl_str.to_string().replace("\"", "");
                let ttl: i32 = match ttl_str.parse() {
                    Ok(num) => num,
                    Err(err) => {
                        log_to_file_and_console(
                            &format!("Failed to convert TTL to number{}", format_err(err)),
                            LogType::Error,
                            &config.log_config,
                        );
                        return Err(());
                    }
                };
                ttl
            }
            None => {
                log_to_file_and_console(
                    "Getting content from dns record failed",
                    LogType::Error,
                    &config.log_config,
                );
                return Err(());
            }
        },
    }))
}
fn is_terminal() -> bool {
    let mut stdin_exists = false;
    let mut stdout_exists = false;
    let mut stderr_exists = false;
    if atty::is(atty::Stream::Stdin) {
        stdin_exists = true;
    }
    if atty::is(atty::Stream::Stdout) {
        stdout_exists = true;
    }
    if atty::is(atty::Stream::Stderr) {
        stderr_exists = true;
    }
    let mut is_terminal = false;
    if stderr_exists && stdout_exists && stdin_exists {
        is_terminal = true;
    }
    is_terminal
}
fn check_for_root() {
    match home_dir() {
        Some(home_dir) => {
            let home_dir_os_string = home_dir.as_os_str();
            let home_dir_os_string = home_dir_os_string.to_owned();
            let home_dir_string = match home_dir_os_string.to_str() {
                Some(string) => string,
                None => panic!("Couldn't convert home directory to string"),
            };
            match home_dir_string {
                "/root" => {
                    panic!("Don't run program as root. Run as some user.");
                }
                _ => {}
            }
        }
        None => panic!("Can't run program without home directory"),
    };
}
fn set_ip(
    ip: &IpAddr,
    name: &String,
    id: &String,
    authentication: &AuthenticationConfig,
    log_config: &LogConfig,
) -> Result<(), CustomError> {
    let ip = ip.to_string();
    let mut request = format!(
        "https://api.cloudflare.com/client/v4/zones/{}/dns_records/",
        &authentication.zone_id
    );
    request.push_str(&id);
    match ureq::patch(&request)
        .set("X-Auth-Email", &authentication.email)
        .set("X-Auth-Key", &authentication.api_key)
        .set("Content-Type", "application/json")
        .send_json(ureq::json!({
          "name": name,
          "content": ip,
        })) {
        Ok(result) => {
            let result_string = match result.into_string() {
                Ok(string) => jsonformat::format(&string, jsonformat::Indentation::Tab),
                Err(err) => {
                    log_to_file_and_console(
                        &format!(
                            "Couldn't convert cloudflare result to string{}",
                            format_err(err)
                        ),
                        LogType::Error,
                        &log_config,
                    );
                    return Err(CustomError::ConvertIntoString);
                }
            };
            match result_string.find("\"success\": true") {
                Some(_) => {}
                None => return Err(CustomError::UnsuccessfullCloudflareRequest(result_string)),
            }
        }
        Err(err) => return Err(CustomError::UReqRequstFailed(err)),
    };
    Ok(())
}
fn get_time(display_date: bool, display_time: bool) -> String {
    let local: DateTime<Local> = Local::now();
    let date = format!(
        "{}/{}/{}",
        convert_to_double_digits(local.day()),
        convert_to_double_digits(local.month()),
        convert_to_double_digits(local.year() as u32)
    );
    let time = format!(
        "{}:{}:{}",
        convert_to_double_digits(local.hour()),
        convert_to_double_digits(local.minute()),
        convert_to_double_digits(local.second())
    );
    if display_date && display_time {
        format!("[{date} {time}] ")
    } else if display_date && !display_time {
        format!("[{date}] ")
    } else if !display_date && display_time {
        format!("[{time}] ")
    } else {
        "".to_string()
    }
}
fn convert_to_double_digits(num: u32) -> String {
    if num >= 10 {
        num.to_string()
    } else {
        format!("0{}", num.to_string())
    }
}
fn write_to_file(
    file_path: &Path,
    contents: String,
    log_config: Option<&LogConfig>,
) -> Result<(), ()> {
    let name = get_path_name(file_path);
    let file: Option<File>;
    if !file_path.exists() {
        file = match File::create(file_path) {
            Ok(file) => Some(file),
            Err(err) => {
                match log_config {
                    Some(log_config) => {
                        log_to_console(
                            &format!("Couldn't create {name}file{}", format_err(err)),
                            LogType::Error,
                            log_config,
                        );
                    }
                    None => println!("Couldn't create {name}file{}", format_err(err)),
                }
                return Err(());
            }
        };
    } else {
        file = Some(
            match OpenOptions::new().write(true).append(true).open(file_path) {
                Ok(file) => file,
                Err(err) => {
                    match log_config {
                        Some(log_config) => {
                            log_to_console(
                                &format!("Couldn't open {name}file{}", format_err(err)),
                                LogType::Error,
                                log_config,
                            );
                        }
                        None => println!("Couldn't open {name}file{}", format_err(err)),
                    }
                    return Err(());
                }
            },
        );
    }
    match file {
        Some(mut file) => {
            match writeln!(file, "{contents}",) {
                Ok(()) => {}
                Err(err) => match log_config {
                    Some(log_config) => {
                        log_to_console(
                            &format!("Couldn't write to {name}file{}", format_err(err)),
                            LogType::Error,
                            log_config,
                        );
                    }
                    None => println!("Couldn't write to {name}file{}", format_err(err)),
                },
            };
            return Ok(());
        }
        None => Err(()),
    }
}
fn get_path_name(path: &Path) -> String {
    match path.file_name() {
        Some(string) => {
            let string = string.to_owned();
            let string = string.to_str();
            match string {
                Some(string) => {
                    format!("{string} ")
                }
                None => "".to_string(),
            }
        }
        None => "".to_string(),
    }
}
fn create_folder(folder_path: &Path, log_config: Option<&LogConfig>) -> Result<(), ()> {
    let name = get_path_name(folder_path);
    if folder_path.is_dir() {
        return Ok(());
    } else {
        match fs::create_dir_all(folder_path) {
            Ok(()) => {
                match log_config {
                    Some(log_config) => {
                        log_to_file_and_console(
                            &format!("Successfully created {name}folder!"),
                            LogType::Log,
                            log_config,
                        );
                    }
                    None => {
                        println!("Successfully created {name}folder!")
                    }
                }
                return Ok(());
            }
            Err(err) => {
                match log_config {
                    Some(log_config) => {
                        log_to_console(
                            &format!("Couldn't create {name}folder{}", format_err(err)),
                            LogType::Error,
                            log_config,
                        );
                    }
                    None => {
                        println!("Couldn't create {name}folder{}", format_err(err))
                    }
                }
                return Err(());
            }
        }
    }
}
fn windows_major_version_number() -> Result<u32, ()> {
    let mut sys = System::new_all();
    sys.refresh_all();
    if let Some(os_version) = sys.os_version() {
        let os_version = os_version.split(" ");
        let major_version_number: Option<u32> = {
            let mut number: Option<u32> = None;
            for string in os_version {
                number = Some(match string.parse() {
                    Ok(num) => num,
                    Err(_) => return Err(()),
                });
                break;
            }
            number
        };
        match major_version_number {
            Some(major_version_number) => return Ok(major_version_number),
            None => Err(()),
        }
    } else {
        Err(())
    }
}
