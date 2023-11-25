use std::{
    error::Error,
    path::{Path, PathBuf},
    process,
};

use dialoguer::{theme::ColorfulTheme, Input, MultiSelect, Select};
use unending_process::{
    create_selection_list, get_log_folder, get_session_number, update_dns_list, Config,
};

use crate::unending_process::{format_err, get_config, log_to_file_and_console, LogType};

mod unending_process;
fn main() {
    let mut args = std::env::args();
    if args.len() > 1 {
        args.next().unwrap();
        let arg = args.next().unwrap();
        if arg == "configure".to_string() {
            let (config, config_path) = get_config();
            main_selection(config, config_path);
        } else {
            println!(
                "There is no command called {}. Did you mean to write configure?",
                arg
            );
        }
    } else {
        unending_process::process();
    }
}
fn main_selection(mut config: Config, config_path: PathBuf) {
    let options = &[
        "Seconds to wait per restart",
        "Authentication",
        "Log Configuration",
        "DNS Records",
        "Exit",
    ];
    let index = match Select::with_theme(&ColorfulTheme::default())
        .items(&options[..])
        .interact()
    {
        Ok(list) => list,
        Err(err) => selection_fail(&config, Box::new(err)),
    };
    match index {
        0 => {
            let seconds: u32 = match Input::with_theme(&ColorfulTheme::default())
                .with_prompt(
                    "The number of seconds to wait every between every dns check and change",
                )
                .interact_text()
            {
                Ok(number) => number,
                Err(err) => selection_fail(&config, Box::new(err)),
            };
            config.seconds_to_wait_per_restart = seconds;
            save_config(&config, &config_path, "seconds to wait per restart");
            main_selection(config, config_path);
        }
        1 => authentication_selection(config, config_path),
        2 => log_config_selection(config, config_path),
        3 => dns_config_selection(config, config_path),
        4 => {
            return;
        }
        _ => out_of_bounds_selection(&config),
    }
}
fn authentication_selection(mut config: Config, config_path: PathBuf) {
    let mut authentication = config.authentication.clone();
    let options = &["Email", "Zone ID", "API Key", "Back", "Exit"];
    let index = match Select::with_theme(&ColorfulTheme::default())
        .items(&options[..])
        .interact()
    {
        Ok(list) => list,
        Err(err) => {
            unending_process::log_to_file_and_console(
                &format!("Failed to select option{}", format_err(err)),
                LogType::Error,
                &config.log_config,
            );
            panic!("Failed to select option");
        }
    };
    match index {
        0 => {
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
            authentication.email = email;
        }
        1 => {
            let zone_id: String = match Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Your zone id")
                .interact_text()
            {
                Ok(zone_id) => zone_id,
                Err(err) => panic!("Couldn't get zone id{}", format_err(err)),
            };
            authentication.zone_id = zone_id;
        }
        2 => {
            let api_key: String = match Input::with_theme(&ColorfulTheme::default())
                .with_prompt("Your API Key")
                .interact_text()
            {
                Ok(api_key) => api_key,
                Err(err) => panic!("Couldn't get API Key{}", format_err(err)),
            };
            authentication.api_key = api_key;
        }
        3 => {
            main_selection(config, config_path);
            return;
        }
        4 => process::exit(0),
        _ => out_of_bounds_selection(&config),
    };
    config.authentication = authentication;
    save_config(&config, &config_path, "authentication");
    authentication_selection(config, config_path);
}
fn log_config_selection(mut config: Config, config_path: PathBuf) {
    let options = &[
        "Log configuration path",
        "Separate logs by session",
        "Display",
        "Show",
        "Back",
        "Exit",
    ];
    let index = match Select::with_theme(&ColorfulTheme::default())
        .items(&options[..])
        .interact()
    {
        Ok(list) => list,
        Err(err) => selection_fail(&config, Box::new(err)),
    };
    match index {
        0 => {
            let path_str: String;
            let mut prompt = None;
            loop {
                let options = &["New Path", "Default", "Back", "Exit"];
                let index: usize;
                match prompt {
                    Some(prompt) => {
                        index = match Select::with_theme(&ColorfulTheme::default())
                            .items(&options[..])
                            .with_prompt(prompt)
                            .interact()
                        {
                            Ok(list) => list,
                            Err(err) => selection_fail(&config, Box::new(err)),
                        };
                    }
                    None => {
                        index = match Select::with_theme(&ColorfulTheme::default())
                            .items(&options[..])
                            .interact()
                        {
                            Ok(list) => list,
                            Err(err) => selection_fail(&config, Box::new(err)),
                        };
                    }
                }
                match index {
                    0 => {
                        let path: String =
                            match Input::with_theme(&ColorfulTheme::default()).interact_text() {
                                Ok(path) => path,
                                Err(err) => selection_fail(&config, Box::new(err)),
                            };
                        let path = match Path::new(&path).canonicalize() {
                            Ok(path) => path,
                            Err(_) => {
                                prompt = Some("The path you entered is invalid");
                                continue;
                            }
                        };
                        let original_path_string = match path.to_str() {
                            Some(string) => string,
                            None => {
                                log_to_file_and_console(
                                    "Failed to convert path to string",
                                    LogType::Error,
                                    &config.log_config,
                                );
                                panic!("Failed to convert path to string");
                            }
                        };
                        if !Path::new(&path).exists() {
                            prompt = Some("The path you entered is invalid");
                            continue;
                        } else {
                            path_str = original_path_string.into();
                            break;
                        }
                    }
                    1 => {
                        path_str = get_log_folder();
                        break;
                    }
                    2 => {
                        log_config_selection(config, config_path);
                        return;
                    }
                    3 => process::exit(0),
                    _ => out_of_bounds_selection(&config),
                }
            }
            config.log_config.log_folder_path = path_str;
            config.log_config.session_number = get_session_number(&config.log_config);
            save_config(&config, &config_path, "the logs folder path");
            log_config_selection(config, config_path);
        }
        1 => {
            match bool_select(&config, "Should log files be separated by session? (every time the program starts, it is starting a new session)") {
                Some(value) => config.log_config.separate_logs_by_session = value,
                None => {
                    log_config_selection(config, config_path);
                    return;
                }
            }
            save_config(&config, &config_path, "separating logs by session");
            log_config_selection(config, config_path);
        }
        2 => display_selection(config, config_path),
        3 => show_selection(config, config_path),
        4 => main_selection(config, config_path),
        5 => process::exit(0),
        _ => out_of_bounds_selection(&config),
    }
}
fn display_selection(mut config: Config, config_path: PathBuf) {
    let options = &["Date", "Time", "Log Type", "Back", "Exit"];
    let index = match Select::with_theme(&ColorfulTheme::default())
        .items(&options[..])
        .interact()
    {
        Ok(list) => list,
        Err(err) => selection_fail(&config, Box::new(err)),
    };
    match index {
        0 => {
            match bool_select(
                &config,
                "[ Should the date be added to the beginning to every new line? -> 24/11/2023 23:40:43] [LOG]",
            ) {
                Some(value) => config.log_config.display.date = value,
                None => {
                    log_config_selection(config, config_path);
                    return;
                }
            }
            save_config(&config, &config_path, "if the date should be shown");
            display_selection(config, config_path);
        }
        1 => {
            match bool_select(
                &config,
                "[24/11/2023 Should the time be added to the beginning to every new line? -> 23:40:43] [LOG]",
            ) {
                Some(value) => config.log_config.display.time = value,
                None => {
                    log_config_selection(config, config_path);
                    return;
                }
            }
            save_config(&config, &config_path, "if the time should be shown");
            display_selection(config, config_path);
        }
        2 => {
            match bool_select(
                &config,
                "[24/11/2023 23:40:43] Should the log type be added to the beginning to every new line? -> [LOG]",
            ) {
                Some(value) => config.log_config.display.log_type = value,
                None => {
                    log_config_selection(config, config_path);
                    return;
                }
            }
            save_config(&config, &config_path, "if the log type should be shown");
            display_selection(config, config_path);
        }
        3 => log_config_selection(config, config_path),
        4 => process::exit(0),
        _ => out_of_bounds_selection(&config),
    }
}
fn show_selection(mut config: Config, config_path: PathBuf) {
    let options = &["Logs", "Warnings", "Errors", "Back", "Exit"];
    let index = match Select::with_theme(&ColorfulTheme::default())
        .items(&options[..])
        .interact()
    {
        Ok(list) => list,
        Err(err) => selection_fail(&config, Box::new(err)),
    };
    match index {
        0 => {
            match bool_select(
                &config,
                "Should logs be printed to the console and written to the log file?",
            ) {
                Some(value) => config.log_config.show.logs = value,
                None => {
                    log_config_selection(config, config_path);
                    return;
                }
            }
            save_config(
                &config,
                &config_path,
                "if logs should be displayed to the console",
            );
            show_selection(config, config_path);
        }
        1 => {
            match bool_select(
                &config,
                "Should warnings be printed to the console and written to the log file?",
            ) {
                Some(value) => config.log_config.show.warnings = value,
                None => {
                    log_config_selection(config, config_path);
                    return;
                }
            }
            save_config(
                &config,
                &config_path,
                "if warnings should be displayed to the console",
            );
            show_selection(config, config_path);
        }
        2 => {
            match bool_select(
                &config,
                "Should errors be printed to the console and written to the log file?",
            ) {
                Some(value) => config.log_config.show.errors = value,
                None => {
                    log_config_selection(config, config_path);
                    return;
                }
            }
            save_config(
                &config,
                &config_path,
                "if errors should be displayed to the console",
            );
            show_selection(config, config_path);
        }
        3 => log_config_selection(config, config_path),
        4 => process::exit(0),
        _ => out_of_bounds_selection(&config),
    }
}
fn dns_config_selection(mut config: Config, config_path: PathBuf) {
    update_dns_list(&mut config, &config_path);
    let ((multiselected, ids), defaults) = create_selection_list(&config.dns_config);
    let selections = match MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select which records need to be synced")
        .items(&multiselected[..])
        .defaults(&defaults[..])
        .interact()
    {
        Ok(list) => list,
        Err(err) => selection_fail(&config, Box::new(err)),
    };
    for record in &mut config.dns_config {
        record.sync = Some(false);
    }
    for selection in selections {
        for record in &mut config.dns_config {
            if record.id == ids[selection] {
                record.sync = Some(true);
            } else if record.sync == None {
                record.sync = Some(false);
            }
        }
    }
    save_config(&config, &config_path, "the DNS records list");
    main_selection(config, config_path);
}
fn bool_select(config: &Config, prompt: &str) -> Option<bool> {
    let options = &["True", "False", "Back", "Exit"];
    let index = match Select::with_theme(&ColorfulTheme::default())
        .items(&options[..])
        .with_prompt(prompt)
        .interact()
    {
        Ok(list) => list,
        Err(err) => selection_fail(&config, Box::new(err)),
    };
    let value: Option<bool>;
    match index {
        0 => value = Some(true),
        1 => value = Some(false),
        2 => {
            return None;
        }
        3 => process::exit(0),
        _ => out_of_bounds_selection(&config),
    };
    value
}
fn save_config(config: &Config, config_path: &PathBuf, name: &str) {
    match config.save_to_json(&config_path) {
        Ok(()) => log_to_file_and_console(
            &format!("Successfully saved {name} to the config file!"),
            LogType::Log,
            &config.log_config,
        ),
        Err(()) => {
            log_to_file_and_console("Failed to save config", LogType::Error, &config.log_config);
            panic!("Failed to save config");
        }
    }
}
fn out_of_bounds_selection(config: &Config) -> ! {
    log_to_file_and_console(
        "Selection index was out of bounds. Unable to proceed",
        LogType::Error,
        &config.log_config,
    );
    process::exit(0)
}
fn selection_fail(config: &Config, err: Box<dyn Error>) -> ! {
    {
        unending_process::log_to_file_and_console(
            &format!("Failed to select option{}", format_err(err)),
            LogType::Error,
            &config.log_config,
        );
        panic!("Failed to select option")
    }
}
#[cfg(test)]
mod test {
    use crate::unending_process;
    #[test]
    fn print_env_vars() {
        use colored::Colorize;
        for (key, value) in std::env::vars() {
            println!("{key}: {value}");
        }
        let colored_string = "hello";
        println!("{}", colored_string.red());
    }
    #[test]
    fn check_for_color_support() {
        if let Some(support) = supports_color::on(supports_color::Stream::Stdout) {
            if support.has_16m {
                println!("16 million (RGB) colors are supported");
            } else if support.has_256 {
                println!("256 colors are supported.");
            } else if support.has_basic {
                println!("Only basic ANSI colors are supported.");
            }
        } else {
            println!("No color support.");
        }
    }
    #[test]
    fn process_test() {
        unending_process::process();
    }
}
