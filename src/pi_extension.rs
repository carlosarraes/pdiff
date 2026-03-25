use std::fs;
use std::io;
use std::path::PathBuf;

const EXTENSION_SOURCE: &str = include_str!("pi_extension_src.ts");

fn extensions_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    Some(home.join(".pi").join("agent").join("extensions"))
}

fn install_dir() -> Option<PathBuf> {
    extensions_dir().map(|d| d.join("pdiff"))
}

pub fn install(target: &str) -> io::Result<()> {
    if target != "pi" {
        eprintln!("Unknown target: {}. Supported: pi", target);
        std::process::exit(1);
    }

    let dir = install_dir().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Could not determine home directory")
    })?;

    fs::create_dir_all(&dir)?;
    fs::write(dir.join("index.ts"), EXTENSION_SOURCE)?;

    eprintln!("Installed pdiff extension to {}", dir.display());
    eprintln!("Restart pi to activate. Or run: pi -e {}", dir.display());

    Ok(())
}

pub fn uninstall(target: &str) -> io::Result<()> {
    if target != "pi" {
        eprintln!("Unknown target: {}. Supported: pi", target);
        std::process::exit(1);
    }

    let dir = install_dir().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "Could not determine home directory")
    })?;

    if !dir.exists() {
        eprintln!("pdiff extension not found at {}", dir.display());
        std::process::exit(0);
    }

    fs::remove_dir_all(&dir)?;
    eprintln!("Uninstalled pdiff extension from {}", dir.display());

    Ok(())
}
