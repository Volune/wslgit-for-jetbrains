#[macro_use]
extern crate lazy_static;
extern crate regex;

use std::collections::{HashMap};
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf, Prefix, PrefixComponent};
use std::process::{Command, Stdio};

use regex::Regex;

const CONFIG_FOLDER: &str = "wslgit-for-jetbrains";
const MAPPING_FILENAME: &str = "mapping.txt";

lazy_static! {
    static ref DRIVE_TO_PATH_MAP: HashMap<String, String> = build_drive_to_path_mapping();
}

fn get_mapping_config_path() -> PathBuf {
    env::var("LOCALAPPDATA")
        .ok()
        .map(|appdata| {
            [
                appdata.as_str(),
                CONFIG_FOLDER,
                MAPPING_FILENAME
            ]
                .iter()
                .collect::<PathBuf>()
        })
        .expect("Cannot compute config path")
}

fn get_drive_letter(pc: &PrefixComponent) -> Option<String> {
    let drive_byte = match pc.kind() {
        Prefix::VerbatimDisk(d) => Some(d),
        Prefix::Disk(d) => Some(d),
        _ => None
    };
    drive_byte.map(|drive_letter| {
        String::from_utf8(vec![drive_letter])
            .expect(&format!("Invalid drive letter: {}", drive_letter))
            .to_lowercase()
    })
}

fn get_prefix_for_drive(drive: String) -> String {
    // todo - lookup mount points
    format!("/mnt/{}", drive)
}

fn translate_path_to_unix(argument: String) -> String {
    lazy_static! {
        static ref ESCAPE_RE: Regex = Regex::new(r"[^a-zA-Z0-9,._+@%/-]").expect("Failed to compile SPACE regex");
    }
    {
        let (argname, arg) = if argument.starts_with("--")
            && argument.contains('=') {
            let parts: Vec<&str> = argument
                .splitn(2, '=')
                .collect();
            (format!("{}=", parts[0]), parts[1])
        } else {
            ("".to_owned(), argument.as_ref())
        };
        let win_path = Path::new(arg);
        if win_path.is_absolute() || win_path.exists() {
            let wsl_path = win_path
                .components()
                .filter_map(|comp| {
                    let comp: Option<String> = match comp {
                        Component::Prefix(prefix_comp) => {
                            let drive_letter = get_drive_letter(&prefix_comp)
                                .expect(&format!("Cannot handle path {:?}", win_path));
                            Some(get_prefix_for_drive(drive_letter))
                        }
                        Component::RootDir => None,
                        _ => comp
                            .as_os_str()
                            .to_str()
                            .map(|s| String::from(ESCAPE_RE.replace_all(s, "\\$0")))
                    };
                    comp
                })
                .collect::<Vec<String>>()
                .join("/");
            return format!("{}{}", &argname, wsl_path);
        }
    }
    argument
}

fn translate_path_to_win(unix_path: String) -> String {
    lazy_static! {
        static ref WSLPATH_RE: Regex =
            Regex::new(r"(?m-u)/mnt/(?P<drive>[A-Za-z])(?P<path>/\S*)")
                .expect("Failed to compile WSLPATH regex");
    }
    String::from(WSLPATH_RE.replace(unix_path.as_str(), "${drive}:${path}"))
}

fn translate_path_to_win_output(line: String) -> String {
    lazy_static! {
        static ref WSLPATH_RE: Regex =
            Regex::new(r"(?m-u)/mnt/(?P<drive>[A-Za-z])(?P<path>/\S*)")
                .expect("Failed to compile WSLPATH regex");
    }
    String::from(WSLPATH_RE.replace_all(line.as_str(), "${drive}:${path}"))
}

fn is_translated_command(arg: String) -> bool {
    const TRANSLATED_CMDS: &[&str] = &["rev-parse", "remote"];
    TRANSLATED_CMDS.contains(&arg.as_str())
}

fn is_version_command(arg: String) -> bool {
    const MATCHES: &[&str] = &["version", "--version"];
    MATCHES.contains(&arg.as_str())
}

fn arg_matching(f: fn(String) -> bool) -> bool {
    env::args().skip(1).position(f).is_some()
}

fn append_version(line: String) -> String {
    line + " wslgit-for-jetbrains." + env!("CARGO_PKG_VERSION")
}

fn build_drive_to_path_mapping() -> HashMap<String, String> {
    let mut drive_to_path_map = HashMap::new();
    let mapping_config_path = get_mapping_config_path();
    if mapping_config_path.exists() {
        let config_file = File::open(mapping_config_path.as_path())
            .expect("Cannot open config file");
        let reader = BufReader::new(config_file);
        for line in reader.lines() {
            if let Ok(line) = line {
                let drive_and_path = line.splitn(2, " ").collect::<Vec<&str>>();
                drive_to_path_map.insert(drive_and_path[0].to_string(), drive_and_path[1].to_string());
            }
        }
    }
    drive_to_path_map
}

fn command_generate_mapping() {
    lazy_static! {
        static ref DRIVE_MAPPING_RE: Regex =
            Regex::new(r"^(?P<drive>[a-zA-Z]):\s+on\s+(?P<path>\S+)")
                .expect("Failed to compile DRIVE_MAPPING regex");
    }
    let mapping_config_path = get_mapping_config_path();
    let config_path_str = mapping_config_path.to_str().expect("Cannot generate config path");
    let mut mount_child = Command::new("wsl")
        .args(&["mount"])
        .stdout( Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("Failed to run 'mount'");
    if let Some(ref mut child_stdout) = mount_child.stdout {
        let child_stdout = BufReader::new(child_stdout);
        let lines_iter = child_stdout
            .lines()
            .filter_map(|l| l.ok());
        let config_folder = mapping_config_path.parent().expect("No config folder");
        fs::create_dir_all(config_folder).expect("Cannot create config folder");
        let mut config_file = File::create(mapping_config_path.as_path())
            .expect(&format!("Cannot create config file {}", config_path_str));
        println!("{}", config_path_str);
        for line in lines_iter {
            if let Some(capture) = DRIVE_MAPPING_RE.captures(&line) {
                let drive = capture.name("drive")
                    .expect("Cannot find capture group 'drive'")
                    .as_str()
                    .to_lowercase();
                let mount_path = capture.name("path")
                    .expect("Cannot find capture group 'path'")
                    .as_str();
                write!(config_file, "{} {}\n", drive, mount_path)
                    .expect("Cannot write to config file");
                println!("{} <-> {}", drive, mount_path);
            }
        }
    }
}

fn command_show_mapping() {
    let mapping_config_path = get_mapping_config_path();
    let config_path_str = mapping_config_path.to_str().expect("Cannot generate config path");
    if mapping_config_path.exists() {
        println!("{}", config_path_str);
        for (drive, unix_path) in DRIVE_TO_PATH_MAP.iter() {
            println!("{} <-> {}", drive, unix_path);
        }
    } else {
        println!("{} not found", config_path_str);
    }
}

fn main() {
    let mut args: Vec<String> = vec![];
    let mut proc_setup;
    let mut opt_transform_output: Option<fn(String) -> String> = None;

    let first_arg = env::args().nth(1).unwrap_or_default();

    if first_arg == "win-show-mapping" {
        command_show_mapping();
        return;
    } else if first_arg == "win-generate-mapping" {
        command_generate_mapping();
        return;
    } else if first_arg == "win-cmd" {
        proc_setup = Command::new("cmd");
        args.push(String::from("/c"));
        args.extend(env::args().skip(2).map(translate_path_to_win));
    } else {
        proc_setup = Command::new("wsl");
        args.push(String::from("git"));
        args.extend(env::args().skip(1).map(translate_path_to_unix));

        // add git commands that must use translate_path_to_win
        if arg_matching(is_translated_command) {
            opt_transform_output = Some(translate_path_to_win_output);
        }
        if arg_matching(is_version_command) {
            opt_transform_output = Some(append_version);
        }

        let wslgit_cmd = translate_path_to_unix(env::args().nth(0).expect("Cannot find args[0]"));
        let mut wsl_env: Vec<String> = vec![];
        for (ref env_key, ref env_val) in env::vars() {
            if env_key.starts_with("GIT_") {
                if env_key == "GIT_EDITOR" {
                    proc_setup.env("GIT_EDITOR", format!("{} win-cmd {}", wslgit_cmd, env_val));
                } else {
                    proc_setup.env(env_key, env_val);
                }
                wsl_env.push(format!("{}/u", env_key));
            }
        }
        if !wsl_env.is_empty() {
            proc_setup.env("WSLENV", wsl_env.join(":"));
        }
    }

    // setup the git subprocess launched inside WSL
    proc_setup
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(if opt_transform_output.is_some() { Stdio::piped() } else { Stdio::inherit() })
        .stderr(Stdio::inherit());

    let mut child = proc_setup
        .spawn()
        .expect(&format!("Failed to execute command '{}'", args.join(" ")));

    if let Some(transform_output) = opt_transform_output {
        if let Some(ref mut child_stdout) = child.stdout {
            let child_stdout = BufReader::new(child_stdout);
            let mut stdout = io::stdout();
            let lines_iter = child_stdout.lines().filter_map(|l| l.ok());
            for line in lines_iter {
                stdout.write_all(transform_output(line).as_bytes()).ok();
            }
            stdout.flush().expect("Failed to flush output");
        }
    }

    let status = child.wait().expect("Failed to wait for command");
    // forward any exit code
    if let Some(exit_code) = status.code() {
        std::process::exit(exit_code);
    }
}


#[test]
fn win_to_unix_path_trans() {
    assert_eq!(
        translate_path_to_unix("d:\\test\\file.txt".to_string()),
        "/mnt/d/test/file.txt");
    assert_eq!(
        translate_path_to_unix("C:\\Users\\test\\a space.txt".to_string()),
        "/mnt/c/Users/test/a\\ space.txt");
}

#[test]
fn unix_to_win_path_trans() {
    assert_eq!(
        translate_path_to_win_output("/mnt/d/some path/a file.md".to_string()),
        "d:/some path/a file.md".to_string());
    assert_eq!(
        translate_path_to_win_output("origin  /mnt/c/path/ (fetch)".to_string()),
        "origin  c:/path/ (fetch)");
    let multiline = "mirror  /mnt/c/other/ (fetch)\nmirror  /mnt/c/other/ (push)\n";
    let multiline_result = "mirror  c:/other/ (fetch)\nmirror  c:/other/ (push)\n";
    assert_eq!(
        &*translate_path_to_win_output(String::from(multiline)),
        &multiline_result[..]);
}

#[test]
fn no_path_translation() {
    assert_eq!(
        &*translate_path_to_win_output(String::from("/mnt/other/file.sh")),
        String::from("/mnt/other/file.sh"));
}

#[test]
fn relative_path_translation() {
    assert_eq!(
        translate_path_to_unix(".\\src\\main.rs".to_string()),
        "./src/main.rs");
}

#[test]
fn long_argument_path_translation() {
    assert_eq!(
        translate_path_to_unix("--file=C:\\some\\path.txt".to_owned()),
        "--file=/mnt/c/some/path.txt");
}
