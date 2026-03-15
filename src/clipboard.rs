use color_eyre::{eyre::eyre, Result};

pub fn copy_to_clipboard(text: &str) -> Result<()> {
    use std::io::Write;
    // Try wl-copy (Wayland), then xclip, then xsel
    let candidates = [
        ("wl-copy", vec![]),
        ("xclip", vec!["-selection", "clipboard"]),
        ("xsel", vec!["--clipboard", "--input"]),
    ];
    for (cmd, args) in &candidates {
        if let Ok(mut child) = std::process::Command::new(cmd)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
            return Ok(());
        }
    }
    Err(eyre!(
        "No clipboard tool found (install xclip, xsel, or wl-copy)"
    ))
}
