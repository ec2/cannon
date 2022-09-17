use anyhow::*;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

fn main() -> Result<()> {
    let task = env::args().nth(1);
    match task.as_ref().map(|it| it.as_str()) {
        Some("test") => test()?,
        Some("build-image") => build_image()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:
test            builds the contracts and runs the tests
build-image     builds the MIPS image ready for use in prover and verifier
"
    )
}

fn test() -> Result<()> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(cargo.clone())
        .current_dir(contracts_root())
        .args(&["build", "--release"])
        .status()?;
    if !status.success() {
        bail!("building cargo build --release");
    }
    let status = Command::new(cargo)
        .current_dir(project_root())
        .args(&["test"])
        .status()?;
    if !status.success() {
        bail!("testing the contracts failed");
    }
    Ok(())
}

fn build_image() -> Result<()> {
    let cargo = env::var("CROSS").unwrap_or_else(|_| "cross".to_string());
    let status = Command::new(cargo.clone())
        .current_dir(contracts_root())
        .args(&["build", "--release", "--target", "mips-unknown-linux-gnu"])
        .status()?;
    if !status.success() {
        bail!("building cargo build --release");
    }
    Ok(())
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

fn contracts_root() -> PathBuf {
    project_root().join("contracts")
}
