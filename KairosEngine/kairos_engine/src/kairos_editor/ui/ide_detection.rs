//! 跨平台 IDE 嗅探与文件打开工具。
//!
//! 自动检测本机已安装的编辑器（VS Code、Zed、Cursor 等），
//! 并按优先级选择可用的编辑器打开文件。
//!
//! ## 策略
//!
//! 1. 扫描 `PATH` 与平台常用安装路径，检测已安装的 IDE。
//! 2. 用 `open::with_detached` 将文件交给指定 IDE 打开。
//!    底层利用各平台原生机制：
//!    - macOS: `open -a <app> <file>`（Launch Services）
//!    - Windows: `ShellExecuteW`
//!    - Linux: `xdg-open`
//!
//! 支持 macOS / Windows / Linux。

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// 已知 IDE 列表（按优先级排序）
// ---------------------------------------------------------------------------

struct IdeDescriptor {
    name: &'static str,
    cli: &'static str,
    fallbacks: &'static [&'static str],
}

const KNOWN_IDES: &[IdeDescriptor] = &[
    IdeDescriptor {
        name: "Cursor",
        cli: "cursor",
        fallbacks: &[
            "/Applications/Cursor.app/Contents/Resources/app/bin/cursor",
            r"C:\Users\%USERNAME%\AppData\Local\Programs\cursor\resources\app\bin\cursor",
            "/opt/cursor/cursor",
        ],
    },
    IdeDescriptor {
        name: "VS Code",
        cli: "code",
        fallbacks: &[
            "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
            r"C:\Program Files\Microsoft VS Code\bin\code.cmd",
            r"C:\Program Files (x86)\Microsoft VS Code\bin\code.cmd",
            "/usr/share/code/bin/code",
            "/snap/bin/code",
        ],
    },
    IdeDescriptor {
        name: "VS Code Insiders",
        cli: "code-insiders",
        fallbacks: &[
            "/Applications/Visual Studio Code - Insiders.app/Contents/Resources/app/bin/code",
            r"C:\Program Files\Microsoft VS Code Insiders\bin\code-insiders.cmd",
            "/usr/share/code-insiders/bin/code-insiders",
        ],
    },
    IdeDescriptor {
        name: "Zed",
        cli: "zed",
        fallbacks: &[
            "/Applications/Zed.app/Contents/MacOS/zed",
            "/Applications/Zed Preview.app/Contents/MacOS/zed",
            "/var/lib/flatpak/app/dev.zed.Zed/current/active/files/bin/zed",
            "/var/lib/flatpak/app/dev.zed.Zed.Preview/current/active/files/bin/zed",
        ],
    },
    IdeDescriptor {
        name: "Windsurf",
        cli: "windsurf",
        fallbacks: &[
            "/Applications/Windsurf.app/Contents/Resources/app/bin/windsurf",
            r"C:\Users\%USERNAME%\AppData\Local\Programs\Windsurf\resources\app\bin\windsurf",
        ],
    },
    IdeDescriptor {
        name: "IntelliJ IDEA",
        cli: "idea",
        fallbacks: &[
            "/Applications/IntelliJ IDEA.app/Contents/MacOS/idea",
            "/Applications/IntelliJ IDEA Ultimate.app/Contents/MacOS/idea",
            "/Applications/IntelliJ IDEA Community Edition.app/Contents/MacOS/idea",
            r"C:\Program Files\JetBrains\IntelliJ IDEA\bin\idea64.exe",
            r"C:\Program Files\JetBrains\IntelliJ IDEA Community Edition\bin\idea64.exe",
            "/opt/jetbrains/idea/bin/idea.sh",
            "/snap/intellij-idea-community/current/bin/idea.sh",
        ],
    },
    IdeDescriptor {
        name: "Fleet",
        cli: "fleet",
        fallbacks: &["/Applications/Fleet.app/Contents/MacOS/fleet"],
    },
];

// ---------------------------------------------------------------------------
// 检测 + 缓存
// ---------------------------------------------------------------------------

/// 检测某个 CLI 命令是否存在于 `PATH` 中。
fn cli_on_path(cli: &str) -> bool {
    std::process::Command::new(cli)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// 展开路径中的 `%USERNAME%` 占位符。
fn expand_placeholders(path: &str) -> String {
    if path.contains("%USERNAME%") {
        let username = std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_default();
        if username.is_empty() {
            return path.to_owned();
        }
        path.replace("%USERNAME%", &username)
    } else {
        path.to_owned()
    }
}

/// 已检测到的 IDE 信息（运行时）。
struct DetectedIde {
    name: &'static str,
    /// 实际可执行路径（CLI 命令名或完整绝对路径）
    binary: String,
}

/// 检测本机所有已安装的 IDE。
fn detect_ides() -> Vec<DetectedIde> {
    let mut available: Vec<DetectedIde> = Vec::new();

    for desc in KNOWN_IDES {
        // 1. 优先尝试 CLI 命令（PATH 中存在）
        if cli_on_path(desc.cli) {
            available.push(DetectedIde {
                name: desc.name,
                binary: desc.cli.to_owned(),
            });
            continue;
        }

        // 2. 遍历平台备用路径
        for fallback in desc.fallbacks {
            let expanded = expand_placeholders(fallback);
            if Path::new(&expanded).exists() {
                available.push(DetectedIde {
                    name: desc.name,
                    binary: expanded,
                });
                break;
            }
        }
    }

    available
}

fn cached_ides() -> &'static Vec<DetectedIde> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Vec<DetectedIde>> = OnceLock::new();
    CACHE.get_or_init(detect_ides)
}

// ---------------------------------------------------------------------------
// 路径处理
// ---------------------------------------------------------------------------

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
    }
}

// ---------------------------------------------------------------------------
// 公共 API
// ---------------------------------------------------------------------------

/// 获取当前系统上所有可用的 IDE 名称列表（按优先级排序）。
pub fn available_ides() -> Vec<&'static str> {
    cached_ides().iter().map(|ide| ide.name).collect()
}

/// 使用当前系统上优先级最高的可用 IDE 打开文件。
///
/// 遍历已检测到的 IDE，用 `open::with_detached` 尝试打开。
/// 如果没有任何 IDE 可用或全部失败，返回 `false`。
pub fn open_file(path: &Path) -> bool {
    let ides = cached_ides();
    let abs_path = absolutize(path);

    if ides.is_empty() {
        log::warn!(
            "No supported IDE detected on this system. \
             Supported: {}",
            KNOWN_IDES
                .iter()
                .map(|i| i.name)
                .collect::<Vec<_>>()
                .join(", ")
        );
        return false;
    }

    for ide in ides {
        match open::with_detached(&abs_path, &ide.binary) {
            Ok(()) => {
                log::info!("Opened '{}' with {}", abs_path.display(), ide.name);
                return true;
            }
            Err(e) => {
                log::debug!(
                    "Failed to open '{}' with {}: {e}",
                    abs_path.display(),
                    ide.name
                );
            }
        }
    }

    log::warn!(
        "All available IDEs failed to open '{}'. Available: {:?}",
        abs_path.display(),
        cached_ides().iter().map(|i| i.name).collect::<Vec<_>>()
    );
    false
}

/// 使用指定名称的 IDE 打开文件。
///
/// `ide_name` 可以是展示名称（如 `"VS Code"`）或 CLI 名称（如 `"code"`）。
pub fn open_file_with(path: &Path, ide_name: &str) -> bool {
    let ides = cached_ides();
    let abs_path = absolutize(path);

    let target = ides.iter().find(|ide| {
        if ide.name.eq_ignore_ascii_case(ide_name) {
            return true;
        }
        // 也支持通过 CLI 命令名查找（如 "code"、"zed"）
        KNOWN_IDES
            .iter()
            .any(|desc| desc.name == ide.name && desc.cli.eq_ignore_ascii_case(ide_name))
    });

    match target {
        Some(ide) => match open::with_detached(&abs_path, &ide.binary) {
            Ok(()) => {
                log::info!(
                    "Opened '{}' with {} (by name)",
                    abs_path.display(),
                    ide.name
                );
                true
            }
            Err(e) => {
                log::warn!(
                    "Failed to open '{}' with '{}': {e}",
                    abs_path.display(),
                    ide_name
                );
                false
            }
        },
        None => {
            log::warn!(
                "IDE '{}' is not installed or not detected. Available: {:?}",
                ide_name,
                available_ides()
            );
            false
        }
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_placeholders_noop() {
        assert_eq!(expand_placeholders("/usr/bin/code"), "/usr/bin/code");
    }

    #[test]
    fn test_expand_placeholders_username() {
        let expanded = expand_placeholders(r"C:\Users\%USERNAME%\AppData\...");
        assert!(!expanded.contains("%USERNAME%"));
        assert!(expanded.starts_with(r"C:\Users\"));
    }

    #[test]
    fn test_detect_ides_returns_some() {
        let ides = detect_ides();
        for ide in &ides {
            assert!(!ide.name.is_empty());
            assert!(!ide.binary.is_empty());
        }
    }
}
