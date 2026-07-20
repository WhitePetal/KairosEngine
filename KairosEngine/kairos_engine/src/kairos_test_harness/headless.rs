use crate::kairos_editor::KairosEngine;
use crate::kairos_test_harness::{bridge::Bridge, test_runner, ws_server};

/// Parsed command-line arguments for the test harness.
pub struct CliArgs {
    pub headless: bool,
    pub test_file: Option<String>,
    pub gen_docs: bool,
}

/// Parse CLI arguments manually (no dependency on clap).
pub fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut result = CliArgs {
        headless: false,
        test_file: None,
        gen_docs: false,
    };

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--headless" => result.headless = true,
            "--gen-docs" => result.gen_docs = true,
            "--test-file" => {
                i += 1;
                if i < args.len() {
                    result.test_file = Some(args[i].clone());
                }
            }
            _ => {}
        }
        i += 1;
    }

    result
}

/// Run the engine in headless mode: no window, no surface, but wgpu
/// adapter + device are created. Executes the specified TOML test file,
/// prints the result as JSON to stdout, and exits with the appropriate
/// exit code (0 on pass, 1 on failure).
pub async fn run(cli: CliArgs) -> Result<(), Box<dyn std::error::Error>> {
    let test_file = cli
        .test_file
        .unwrap_or_else(|| {
            eprintln!("Error: --test-file is required in headless mode");
            std::process::exit(1);
        });

    log::info!("KairosEngine headless mode — running test: {test_file}");

    // --- Engine init (no window needed) ---
    let egui_ctx = egui::Context::default();
    let mut engine = KairosEngine::new(&egui_ctx)?;

    // --- wgpu init (adapter + device, no surface) ---
    let (_adapter, _device, _queue) = create_headless_wgpu().await;
    log::info!("wgpu adapter and device created in headless mode");

    // --- Bridge ---
    let mut bridge = Bridge::new(256);

    // --- WS server (still available for interactive use) ---
    let ws_tx = bridge.sender();
    tokio::spawn(async move {
        ws_server::start(ws_tx, 9999).await;
    });

    // --- Test runner ---
    let test_tx = bridge.sender();
    let test_handle = tokio::spawn(async move {
        test_runner::run_test_file(&test_file, test_tx).await
    });

    // --- Main loop: drain bridge until test completes ---
    loop {
        bridge.drain(&mut engine);

        if test_handle.is_finished() {
            break;
        }

        // Yield to let tokio worker threads process WS messages
        // and the test runner make progress.
        tokio::task::yield_now().await;
    }

    // Final drain for any remaining commands
    bridge.drain(&mut engine);

    // --- Report ---
    let test_result = test_handle
        .await
        .map_err(|e| format!("test runner panicked: {e}"))?;

    let json = serde_json::to_string_pretty(&test_result)
        .unwrap_or_else(|_| format!("{:?}", test_result));
    println!("{json}");

    if test_result.status == "passed" {
        Ok(())
    } else {
        std::process::exit(1);
    }
}

/// Create a wgpu adapter and device without a surface (headless-safe).
async fn create_headless_wgpu() -> (wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None, // headless: no surface
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to find a suitable wgpu adapter for headless mode");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .expect("Failed to create wgpu device in headless mode");

    (adapter, device, queue)
}
