/// Describes a test command available to agents via the test harness.
struct CommandDoc {
    /// Command name as used in TOML `target` field (e.g. "system.ping").
    name: &'static str,
    /// Human-readable description (Chinese).
    description: &'static str,
    /// Argument schema description.
    args: Option<&'static str>,
    /// Mode availability.
    mode: &'static str,
}

/// Generate test-harness-commands.md from the dispatch table and
/// write it to `docs/ai/test-harness-commands.md`.
pub fn generate() -> Result<(), Box<dyn std::error::Error>> {
    let commands = all_commands();

    let mut md = String::new();

    // Header
    md.push_str("# 测试指令参考 (Test Harness Commands)\n\n");
    md.push_str("本文档由代码自动生成，列出 AI agent 可通过 TOML 测试文件调用的所有引擎命令。\n");
    md.push_str("生成方式: `cargo run --features test-harness -- --gen-docs`\n\n");

    // Call commands
    md.push_str("## Call 命令 (`action = \"call\"`)\n\n");
    md.push_str("用于触发引擎操作。在 TOML 中：\n\n");
    md.push_str("```toml\n[[step]]\naction = \"call\"\ntarget = \"<命令名>\"\nargs = { ... }\n```\n\n");
    md.push_str("| 命令 | 描述 | 参数 | 模式 |\n");
    md.push_str("|------|------|------|------|\n");

    for cmd in &commands {
        if cmd.name.contains("error") || cmd.name.contains("missing") {
            continue; // skip error cases
        }
        let args = cmd.args.unwrap_or("无");
        md.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            cmd.name, cmd.description, args, cmd.mode
        ));
    }

    // Call commands continued
    md.push_str("\n### 可用 Call 命令\n\n");

    for cmd in &commands {
        if cmd.name.contains("error") || cmd.name.contains("missing") {
            continue;
        }
        md.push_str(&format!("#### `{}`\n\n", cmd.name));
        md.push_str(&format!("{}。\n\n", cmd.description));
        md.push_str(&format!("- **模式**: {}\n", cmd.mode));
        if let Some(args) = cmd.args {
            md.push_str(&format!("- **参数**: {}\n", args));
        }
        md.push('\n');

        // Example
        md.push_str("```toml\n");
        md.push_str("[[step]]\n");
        md.push_str(&format!("action = \"call\"\n"));
        md.push_str(&format!("target = \"{}\"\n", cmd.name));
        if cmd.args.is_some() && cmd.name != "system.ping" {
            md.push_str(&format!(
                "args = {{ {} }}\n",
                cmd.args.unwrap().replace(", ", ", ")
            ));
        }
        md.push_str("```\n\n");
    }

    // Assert commands
    md.push_str("## Assert 命令 (`action = \"assert\"`)\n\n");
    md.push_str("用于验证引擎状态。在 TOML 中：\n\n");
    md.push_str("```toml\n[[step]]\naction = \"assert\"\ntarget = \"<断言名>\"\nargs = { ... }\n```\n\n");

    let assertions = [
        ("no_crash", "断言自上次 `no_crash` 检查以来引擎未崩溃或出错", "无", "both"),
        ("ecs_query", "查询 ECS World。v1 支持 `query = \"all\"` 配合 `expect = \"count >= N\"` 等条件表达式", "query (string), expect (string)", "both"),
        ("resource_exists", "断言指定路径的文件或资源存在", "path (string)", "both"),
        ("log_contains", "断言引擎日志缓冲区包含指定模式字符串", "pattern (string)", "both"),
        ("wgpu_valid", "断言 GPU 资源有效。v1 为 stub，始终通过", "resource_type (string, 可选)", "windowed"),
        ("toml_value_equals", "断言 TOML 文件中指定 key 的值等于期望值", "file (string), key (string), value (string)", "both"),
    ];

    md.push_str("| 断言 | 描述 | 参数 | 模式 |\n");
    md.push_str("|------|------|------|------|\n");
    for (name, desc, args, mode) in &assertions {
        md.push_str(&format!("| `{name}` | {desc} | {args} | {mode} |\n"));
    }

    md.push_str("\n### 可用 Assert 命令\n\n");
    for (name, desc, args, mode) in &assertions {
        md.push_str(&format!("#### `{name}`\n\n"));
        md.push_str(&format!("{desc}。\n\n"));
        md.push_str(&format!("- **模式**: {mode}\n"));
        md.push_str(&format!("- **参数**: {args}\n\n"));

        md.push_str("```toml\n");
        md.push_str("[[step]]\n");
        md.push_str(&format!("action = \"assert\"\n"));
        md.push_str(&format!("target = \"{name}\"\n"));
        if *args != "无" {
            // Show an example arg
            let example_arg = match *name {
                "ecs_query" => "args = { query = \"all\", expect = \"count >= 1\" }\n",
                "resource_exists" => "args = { path = \"path/to/file.asset\" }\n",
                "log_contains" => "args = { pattern = \"error message\" }\n",
                "wgpu_valid" => "args = { resource_type = \"Texture\" }\n",
                "toml_value_equals" => "args = { file = \"path/to/file.texture\", key = \"format\", value = \"BC7\" }\n",
                _ => "",
            };
            if !example_arg.is_empty() {
                md.push_str(example_arg);
            }
        }
        md.push_str("```\n\n");
    }

    // Input commands
    md.push_str("## Input 命令 (`action = \"input\"`)\n\n");
    md.push_str("用于向引擎注入键盘/鼠标事件。在 TOML 中：\n\n");
    md.push_str("```toml\n[[step]]\naction = \"input\"\nargs = { device = \"...\", event = \"...\", ... }\n```\n\n");

    md.push_str("### 键盘输入\n\n");
    md.push_str("| 参数 | 值 | 说明 |\n");
    md.push_str("|------|-----|------|\n");
    md.push_str("| `device` | `\"keyboard\"` | 设备类型 |\n");
    md.push_str("| `event` | `\"press\"` / `\"release\"` | 按下或释放 |\n");
    md.push_str("| `key` | `\"W\"`, `\"A\"`, `\"S\"`, `\"D\"` | 按键名 |\n\n");

    md.push_str("```toml\n[[step]]\naction = \"input\"\nargs = { device = \"keyboard\", event = \"press\", key = \"W\" }\n```\n\n");

    md.push_str("### 鼠标输入\n\n");
    md.push_str("**点击：**\n\n");
    md.push_str("| 参数 | 值 | 说明 |\n");
    md.push_str("|------|-----|------|\n");
    md.push_str("| `device` | `\"mouse\"` | 设备类型 |\n");
    md.push_str("| `event` | `\"click\"` | 点击事件 |\n");
    md.push_str("| `button` | `\"Left\"` / `\"Right\"` | 鼠标按键 |\n\n");

    md.push_str("```toml\n[[step]]\naction = \"input\"\nargs = { device = \"mouse\", event = \"click\", button = \"Left\" }\n```\n\n");

    md.push_str("**移动：**\n\n");
    md.push_str("| 参数 | 值 | 说明 |\n");
    md.push_str("|------|-----|------|\n");
    md.push_str("| `device` | `\"mouse\"` | 设备类型 |\n");
    md.push_str("| `event` | `\"move\"` | 移动事件 |\n");
    md.push_str("| `x` | 数值 | 屏幕 X 坐标 |\n");
    md.push_str("| `y` | 数值 | 屏幕 Y 坐标 |\n\n");

    md.push_str("```toml\n[[step]]\naction = \"input\"\nargs = { device = \"mouse\", event = \"move\", x = 320.0, y = 240.0 }\n```\n\n");

    // Write to file
    let out_dir = std::path::Path::new("docs/ai");
    std::fs::create_dir_all(out_dir)?;
    let out_path = out_dir.join("test-harness-commands.md");
    std::fs::write(&out_path, md)?;

    log::info!(
        "Generated test-harness-commands.md at {}",
        out_path.display()
    );
    Ok(())
}

/// Returns the complete list of available test commands.
fn all_commands() -> Vec<CommandDoc> {
    vec![
        CommandDoc {
            name: "system.ping",
            description: "连通性测试命令，始终返回成功",
            args: None,
            mode: "both",
        },
        CommandDoc {
            name: "system.query_widget",
            description: "查询指定 ID 的 widget 屏幕坐标（需要 windowed 模式）",
            args: Some("id (string)"),
            mode: "windowed",
        },
        CommandDoc {
            name: "texture_inspector.set_format",
            description: "设置纹理格式（例如 BC7, R8Unorm）",
            args: Some("format (string)"),
            mode: "windowed",
        },
        CommandDoc {
            name: "texture_inspector.apply",
            description: "应用 inspector 中的修改并保存到文件",
            args: None,
            mode: "windowed",
        },
        // Future commands will be added here as they are registered
        // in the dispatch table. Also see the `input` action below which
        // is a dedicated action type (not a call target).
    ]
}
