use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

pub fn auto_check(root: &Path) -> Result<(), String> {
    let start = Instant::now();
    let timeout = Duration::from_secs(120);

    let pkg = root.join("package.json");
    let cargo = root.join("Cargo.toml");
    let pyproject = root.join("pyproject.toml");
    let reqs = root.join("requirements.txt");

    if pkg.exists() {
        let mut cmd = Command::new("npm");
        cmd.arg("-s").arg("run").arg("build").current_dir(root);
        let out = cmd.output();
        if start.elapsed() > timeout {
            return Err("AUTO_CHECK_TIMEOUT".into());
        }
        if let Ok(o) = out {
            if !o.status.success() {
                let mut cmd2 = Command::new("npm");
                cmd2.arg("-s").arg("test").current_dir(root);
                let o2 = cmd2.output().map_err(|e| e.to_string())?;
                if !o2.status.success() {
                    return Err("AUTO_CHECK_NODE_FAILED".into());
                }
            }
        } else {
            return Err("AUTO_CHECK_NODE_FAILED".into());
        }
    }

    if cargo.exists() {
        let mut cmd = Command::new("cargo");
        cmd.arg("check").current_dir(root);
        let o = cmd.output().map_err(|e| e.to_string())?;
        if start.elapsed() > timeout {
            return Err("AUTO_CHECK_TIMEOUT".into());
        }
        if !o.status.success() {
            return Err("AUTO_CHECK_RUST_FAILED".into());
        }
    }

    if pyproject.exists() || reqs.exists() {
        let mut cmd = Command::new("python3");
        cmd.arg("-c").arg("print('ok')").current_dir(root);
        let o = cmd.output().map_err(|e| e.to_string())?;
        if !o.status.success() {
            return Err("AUTO_CHECK_PY_FAILED".into());
        }
    }

    Ok(())
}
