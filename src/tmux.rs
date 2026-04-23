use std::io::{self, Write};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct TmuxPane {
    pub id: String,
    pub label: String,
}

pub fn in_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

pub fn self_pane_id() -> Option<String> {
    std::env::var("TMUX_PANE").ok()
}

pub fn list_panes() -> io::Result<Vec<TmuxPane>> {
    let output = Command::new("tmux")
        .args([
            "list-panes",
            "-a",
            "-F",
            "#{pane_id}\t#{session_name}:#{window_index}.#{pane_index}\t#{window_name}\t#{pane_current_command}",
        ])
        .output()?;

    if !output.status.success() {
        return Err(io::Error::other(format!(
            "tmux list-panes failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let self_id = self_pane_id();
    let text = String::from_utf8_lossy(&output.stdout);
    let panes = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\t');
            let id = parts.next()?.to_string();
            let target = parts.next()?;
            let window = parts.next()?;
            let cmd = parts.next()?;
            Some(TmuxPane {
                label: format!("{}  {}  {}  [{}]", id, target, window, cmd),
                id,
            })
        })
        .filter(|p| self_id.as_deref() != Some(&p.id))
        .collect();

    Ok(panes)
}

pub fn pane_exists(id: &str) -> bool {
    Command::new("tmux")
        .args(["display-message", "-p", "-t", id, "#{pane_id}"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn send_to_pane(target: &str, text: &str) -> io::Result<()> {
    let buffer_name = "pi-diff-send";

    let mut child = Command::new("tmux")
        .args(["load-buffer", "-b", buffer_name, "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| io::Error::other("failed to open tmux stdin"))?
        .write_all(text.as_bytes())?;
    let load = child.wait_with_output()?;
    if !load.status.success() {
        return Err(io::Error::other(format!(
            "tmux load-buffer failed: {}",
            String::from_utf8_lossy(&load.stderr)
        )));
    }

    let paste = Command::new("tmux")
        .args(["paste-buffer", "-b", buffer_name, "-t", target, "-d"])
        .output()?;
    if !paste.status.success() {
        return Err(io::Error::other(format!(
            "tmux paste-buffer failed: {}",
            String::from_utf8_lossy(&paste.stderr)
        )));
    }

    Ok(())
}
