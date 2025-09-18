use std::{
    collections::HashMap,
    env, io,
    path::{Path, PathBuf},
    process::Command,
};

/// Holds key/value pairs parsed from "R CMD config --all".
#[derive(Debug)]
struct ConfigVariables {
    map: HashMap<String, String>,
}

impl ConfigVariables {
    fn get_r_cmd_config(&self, key: &str) -> String {
        self.map.get(key).cloned().unwrap_or_default()
    }
}

fn get_r_home() -> String {
    // Indicate that we're trying to find R_HOME from the environment.
    print!("Trying to find R_HOME in environment...");
    if let Ok(r_home) = env::var("R_HOME") {
        println!("{}", r_home);
        return r_home;
    } else {
        println!("unsuccessful!");
    }

    // Inform that R_HOME was not found in the environment and we're trying `R RHOME`.
    print!("R_HOME not found in environment, trying `R RHOME` command...");

    let output = Command::new("R")
        .arg("RHOME")
        .output()
        .expect("Failed to execute `R RHOME` command");

    if output.status.success() {
        let r_home = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if !r_home.is_empty() {
            println!("success: {}", r_home);
            return r_home;
        }
    }

    // If both methods fail, panic with an error message.
    panic!("Could not determine R_HOME: it is not set in the environment and `R RHOME` did not return a valid value.");
}

/// Run `R CMD config --all` using the provided R executable path and return its stdout as a String.
fn r_cmd_config(r_binary: &Path) -> io::Result<String> {
    let output = Command::new(r_binary)
        .args(&["CMD", "config", "--all"])
        .output()?;
    if !output.stderr.is_empty() {
        println!("> {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Build the configuration map by invoking R commands.
fn build_r_cmd_configs() -> ConfigVariables {
    let r_home = get_r_home();

    // Determine the R executable path.
    let r_binary: PathBuf = if cfg!(target_os = "windows") {
        // On Windows R is typically installed in a subdirectory.
        // Try the "x64" folder first (for 64-bit installations), then fall back.
        let candidate = Path::new(&r_home).join("bin").join("x64").join("R.exe");
        if candidate.exists() {
            candidate
        } else {
            Path::new(&r_home).join("bin").join("R.exe")
        }
    } else {
        Path::new(&r_home).join("bin").join("R")
    };

    let configs = r_cmd_config(&r_binary).unwrap_or_default();
    let mut rcmd_config_map = HashMap::new();

    // Parse the output, expecting lines of the form KEY=VALUE.
    for line in configs.lines() {
        // Stop if we reach comments (the R output sometimes appends comments).
        if line.starts_with("##") {
            break;
        }
        let parts: Vec<&str> = line.split('=').map(str::trim).collect();
        if parts.len() == 2 {
            rcmd_config_map.insert(parts[0].to_string(), parts[1].to_string());
        }
    }
    ConfigVariables {
        map: rcmd_config_map,
    }
}

/// Given a list of strings (such as BLAS, LAPACK, etc. flags),
/// extract library paths (starting with "-L") and libraries (starting with "-l").
fn get_libs_and_paths(strings: &[String]) -> (Vec<String>, Vec<String>) {
    let mut paths = Vec::new();
    let mut libs = Vec::new();
    for s in strings {
        for part in s.split_whitespace() {
            if part.starts_with("-L") {
                paths.push(part[2..].trim_matches('"').to_string());
	    } else if part.starts_with("-l") {
		let lib = &part[2..];
		if lib == "flang_rt.runtime" {
		    // LLVM Flang exposes a virtual token that needs real libs.
		    libs.push("FortranRuntime".to_string());
		    libs.push("FortranDecimal".to_string());
		} else {
		    libs.push(lib.to_string());
		}
	    }		
        }
    }
    (paths, libs)
}

fn main() {
    let r_configs = build_r_cmd_configs();
    let config_strings = [
        r_configs.get_r_cmd_config("BLAS_LIBS"),
        r_configs.get_r_cmd_config("LAPACK_LIBS"),
        r_configs.get_r_cmd_config("FLIBS"),
    ];
    let (lib_paths, libs) = get_libs_and_paths(&config_strings);

    // Emit link search paths. Only output those that exist.
    for path in lib_paths {
        if Path::new(&path).exists() {
            println!("cargo:rustc-link-search={}", path);
            eprintln!("cargo:rustc-link-search={}", path);
        }
    }
    // Emit libraries for the linker.
    for lib in libs {
        println!("cargo:rustc-link-lib=dylib={}", lib);
        eprintln!("cargo:rustc-link-lib=dylib={}", lib);
    }
    println!("cargo:rerun-if-changed=build.rs");
}
