//! OnlyAgent Screencap — Screenshot capture agent for OnlyAgent Hands.
//!
//! This daemon runs on the host machine alongside the OnlyKey device.
//! It periodically captures screenshots and uploads them to the gateway
//! for AI visual reasoning.
//!
//! Platform support:
//! - macOS: `screencapture` CLI
//! - Linux: `scrot` or `gnome-screenshot`
//! - Windows: PowerShell + .NET (System.Windows.Forms.Screen)

use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "onlyagent-screencap",
    about = "Screenshot capture agent for OnlyAgent Hands"
)]
struct Cli {
    /// Gateway URL (e.g., https://localhost:10255)
    #[arg(long, env = "GATEWAY_URL")]
    gateway_url: String,

    /// Session-scoped agent token
    #[arg(long, env = "AGENT_TOKEN")]
    agent_token: String,

    /// Session ID
    #[arg(long, env = "SESSION_ID")]
    session_id: String,

    /// Current step ID (updated by gateway)
    #[arg(long, env = "STEP_ID", default_value = "")]
    step_id: String,

    /// Capture interval in seconds
    #[arg(long, default_value = "3")]
    interval: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    info!(
        gateway = %cli.gateway_url,
        session_id = %cli.session_id,
        interval_secs = cli.interval,
        "starting screenshot capture agent"
    );

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true) // dev mode
        .build()
        .context("building HTTP client")?;

    let mut interval = tokio::time::interval(Duration::from_secs(cli.interval));

    loop {
        interval.tick().await;

        // Check if session is still active
        let session_check = client
            .get(format!(
                "{}/v1/hands/sessions/{}",
                cli.gateway_url, cli.session_id
            ))
            .header("Authorization", format!("Bearer {}", cli.agent_token))
            .send()
            .await;

        match session_check {
            Ok(resp) if resp.status().is_success() => {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                let status = body["status"].as_str().unwrap_or("unknown");
                if status == "closed" {
                    info!("session closed — exiting");
                    break;
                }
            }
            Ok(resp) => {
                warn!(status = %resp.status(), "session check failed");
                continue;
            }
            Err(e) => {
                warn!(error = %e, "session check error");
                continue;
            }
        }

        // Capture screenshot
        let screenshot_path = capture_screenshot().await;
        let screenshot_path = match screenshot_path {
            Ok(path) => path,
            Err(e) => {
                warn!(error = %e, "screenshot capture failed");
                continue;
            }
        };

        // Read file
        let image_data = match tokio::fs::read(&screenshot_path).await {
            Ok(data) => data,
            Err(e) => {
                warn!(error = %e, path = %screenshot_path, "failed to read screenshot");
                continue;
            }
        };

        // Upload
        let form = reqwest::multipart::Form::new()
            .text("session_id", cli.session_id.clone())
            .text("step_id", cli.step_id.clone())
            .text("width", "0")
            .text("height", "0")
            .part(
                "image",
                reqwest::multipart::Part::bytes(image_data)
                    .file_name("screenshot.png")
                    .mime_str("image/png")
                    .unwrap(),
            );

        match client
            .post(format!("{}/v1/hands/screenshots", cli.gateway_url))
            .header("Authorization", format!("Bearer {}", cli.agent_token))
            .multipart(form)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                info!("screenshot uploaded successfully");
            }
            Ok(resp) => {
                warn!(status = %resp.status(), "screenshot upload failed");
            }
            Err(e) => {
                warn!(error = %e, "screenshot upload error");
            }
        }

        // Cleanup temp file
        let _ = tokio::fs::remove_file(&screenshot_path).await;
    }

    Ok(())
}

/// Capture a screenshot using platform-native tools.
/// Returns the path to the temporary screenshot file.
async fn capture_screenshot() -> Result<String> {
    let tmp_path = format!("/tmp/onlyagent_screenshot_{}.png", std::process::id());

    if cfg!(target_os = "macos") {
        Command::new("screencapture")
            .args(["-x", &tmp_path])
            .status()
            .context("executing screencapture")?;
    } else if cfg!(target_os = "linux") {
        // Try scrot first, fall back to gnome-screenshot
        let result = Command::new("scrot").args([&tmp_path]).status();
        if result.is_err() {
            Command::new("gnome-screenshot")
                .args(["-f", &tmp_path])
                .status()
                .context("executing gnome-screenshot")?;
        }
    } else {
        // Windows: use PowerShell
        Command::new("powershell")
            .args([
                "-Command",
                &format!(
                    "Add-Type -AssemblyName System.Windows.Forms; \
                     [System.Windows.Forms.Screen]::PrimaryScreen | Out-Null; \
                     $bmp = New-Object System.Drawing.Bitmap( \
                       [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width, \
                       [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height); \
                     $graphics = [System.Drawing.Graphics]::FromImage($bmp); \
                     $graphics.CopyFromScreen(0, 0, 0, 0, $bmp.Size); \
                     $bmp.Save('{}');",
                    tmp_path
                ),
            ])
            .status()
            .context("executing powershell screenshot")?;
    }

    Ok(tmp_path)
}
