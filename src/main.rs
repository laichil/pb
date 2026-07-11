use anyhow::{Context, Result};
use arboard::Clipboard;
use colored::Colorize;
use glob::glob;
use regex::Regex;
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use zhconv::{Variant, zhconv};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BACKUP_SUFFIX: &str = ".bak";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() == 1 {
        // pb (no args) → stdin to clipboard
        let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
        copy_source_to_clipboard(&mut clipboard, "-", None)?;
        println!("Stdin content copied to clipboard");
        return Ok(());
    }

    match args[1].as_str() {
        "--help" | "-h" => {
            print_usage();
            return Ok(());
        }
        "--version" | "-V" => {
            println!("pb {}", VERSION);
            return Ok(());
        }
        "log" => {
            // Parse flags: -f (force/no-preview) + optional positional argument (text to replace)
            let mut force = false;
            let mut idx = 2;
            let mut replace_text: Option<String> = None;

            while idx < args.len() {
                match args[idx].as_str() {
                    "-f" | "--force" => {
                        force = true;
                        idx += 1;
                    }
                    arg if !arg.starts_with('-') => {
                        // First non-flag argument = text to replace
                        replace_text = Some(arg.to_string());
                        idx += 1;
                        break; // consume only first positional arg; ignore extras
                    }
                    unknown => {
                        return Err(anyhow::anyhow!(
                            "Unknown flag for 'log': {}\nTry 'pb --help' for usage",
                            unknown
                        ));
                    }
                }
            }

            // Determine replacement target: explicit arg or auto-detected username
            let target = if let Some(explicit) = replace_text {
                if explicit.trim().is_empty() {
                    return Err(anyhow::anyhow!("Replacement text cannot be empty"));
                }
                explicit
            } else {
                env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .context("Could not auto-detect username (USER/USERNAME env vars missing).\nTry: pb log yourname")?
            };

            // Read stdin content
            let mut buffer = String::new();
            io::stdin()
                .read_to_string(&mut buffer)
                .context("Failed to read stdin (build log)")?;

            if buffer.is_empty() {
                println!("{} No input received on stdin", "⊘".yellow());
                return Ok(());
            }

            // Sanitize: replace target text with 'root' using word boundaries
            let pattern = format!(r"\b{}\b", regex::escape(&target));
            let re = Regex::new(&pattern).context("Failed to compile replacement regex")?;
            let sanitized = re.replace_all(&buffer, "root").to_string();
            let replaced_count = buffer.matches(&target).count();

            // Show preview unless forced (non-blocking — clipboard operation)
            if !force {
                print_log_sanitization_preview(&buffer, &sanitized, &target, replaced_count);
            }

            // Copy to clipboard
            let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
            clipboard
                .set_text(&sanitized)
                .context("Failed to set clipboard content")?;

            println!(
                "{} Log sanitized ({} → root, {} occurrences) → clipboard {}",
                "✓".green(),
                target.bold(),
                replaced_count,
                format!("({} chars)", sanitized.chars().count()).dimmed()
            );
        }
        "lrc" => {
            // Parse lrc subcommand flags/options
            let mut recursive = false;
            let mut pattern = "*.lrc".to_string();
            let mut variant: Option<Variant> = None;
            let mut in_place = false;
            let mut force = false; // ← NEW: track -f flag
            let mut idx = 2;

            while idx < args.len() {
                match args[idx].as_str() {
                    "-r" | "--recursive" => {
                        recursive = true;
                        idx += 1;
                    }
                    "-p" | "--pattern" => {
                        if idx + 1 < args.len() {
                            pattern = args[idx + 1].clone();
                            idx += 2;
                        } else {
                            return Err(anyhow::anyhow!("Pattern required after -p"));
                        }
                    }
                    "-i" | "--in-place" => {
                        in_place = true;
                        idx += 1;
                    }
                    "-f" | "--force" => {
                        // ← NEW: parse -f flag
                        force = true;
                        idx += 1;
                    }
                    "tw" => {
                        variant = Some(Variant::ZhTW);
                        idx += 1;
                    }
                    "cn" => {
                        variant = Some(Variant::ZhCN);
                        idx += 1;
                    }
                    unknown => {
                        // Explicit error for unknown flags (better UX than silent break)
                        return Err(anyhow::anyhow!(
                            "Unknown flag for 'lrc': {}\nTry 'pb --help' for usage",
                            unknown
                        ));
                    }
                }
            }

            // Find matching .lrc files
            let search_pattern = if recursive {
                format!("**/{}", pattern)
            } else {
                pattern.clone()
            };

            let files: Vec<String> = glob(&search_pattern)
                .map_err(|e| anyhow::anyhow!("Invalid pattern '{}': {}", pattern, e))?
                .filter_map(Result::ok)
                .filter_map(|p| p.to_str().map(|s| s.to_string()))
                .collect();

            if files.is_empty() {
                return Err(anyhow::anyhow!(
                    "No matching files found for pattern: {}{}",
                    if recursive { " (recursive)" } else { "" },
                    if pattern == "*.lrc" {
                        "\nTry: pb lrc -p '*.txt'  # search other extensions"
                    } else {
                        ""
                    }
                ));
            }

            // Launch fzf with file list
            let selected = select_file_with_fzf(&files)?;
            if selected.is_empty() {
                println!("{} Selection cancelled", "⊘".yellow());
                return Ok(());
            }

            if in_place {
                // In-place edit mode — respect -f flag for preview/prompt
                let v = variant.unwrap_or(Variant::ZhTW); // default to TW if unspecified
                convert_file_in_place(&selected, v, !force)?; // ← FIXED: pass !force
                println!(
                    "{} {} → {} {}",
                    "✓".green(),
                    selected.bold(),
                    if v == Variant::ZhTW {
                        "Traditional Chinese (zh-TW)"
                    } else {
                        "Simplified Chinese (zh-CN)"
                    },
                    format!(
                        "(file only • clipboard untouched • backup: {}.bak)",
                        selected
                    )
                    .dimmed()
                );
            } else {
                // Clipboard mode (existing behavior)
                let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
                let content = fs::read_to_string(&selected)
                    .with_context(|| format!("Failed to read: {}", selected))?;

                let final_content = if let Some(v) = variant {
                    let converted = zhconv(&content, v);
                    if !force {
                        // ← Show preview unless -f used
                        print_conversion_preview(&content, &converted, &selected);
                    }
                    converted
                } else {
                    content
                };

                clipboard
                    .set_text(final_content)
                    .context("Failed to set clipboard")?;

                println!(
                    "{} {} {}",
                    "📋".cyan(),
                    if variant.is_some() {
                        format!(
                            "→ {}",
                            if variant == Some(Variant::ZhTW) {
                                "Traditional Chinese (zh-TW)"
                            } else {
                                "Simplified Chinese (zh-CN)"
                            }
                        )
                    } else {
                        "copied to clipboard".to_string()
                    },
                    selected.dimmed()
                );
            }
        }
        "tw" | "cn" => {
            let variant = if args[1] == "tw" {
                Variant::ZhTW
            } else {
                Variant::ZhCN
            };

            // Parse flags: -i (in-place), -f (force/no-preview)
            let mut in_place = false;
            let mut force = false;
            let mut filepath: Option<String> = None;
            let mut idx = 2;

            while idx < args.len() {
                match args[idx].as_str() {
                    "-i" | "--in-place" => {
                        in_place = true;
                        idx += 1;
                    }
                    "-f" | "--force" => {
                        force = true;
                        idx += 1;
                    }
                    arg if !arg.starts_with('-') => {
                        filepath = Some(arg.to_string());
                        break;
                    }
                    unknown => {
                        return Err(anyhow::anyhow!("Unknown flag: {}", unknown));
                    }
                }
            }

            if in_place {
                // In-place edit mode (with backup)
                let path = filepath
                    .as_deref()
                    .context("Filename required for in-place editing (-i)")?;
                convert_file_in_place(path, variant, !force)?;
                println!(
                    "{} {} → {} and saved to original file",
                    "✓".green(),
                    path,
                    if variant == Variant::ZhTW {
                        "Traditional Chinese (zh-TW)"
                    } else {
                        "Simplified Chinese (zh-CN)"
                    }
                );
            } else {
                // Clipboard mode (existing behavior)
                let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
                if let Some(path) = filepath {
                    convert_source_to_clipboard(&mut clipboard, &path, variant)?;
                    println!(
                        "Converted {} → {} and copied to clipboard",
                        path,
                        if variant == Variant::ZhTW {
                            "Traditional Chinese (zh-TW)"
                        } else {
                            "Simplified Chinese (zh-CN)"
                        }
                    );
                } else {
                    convert_clipboard(&mut clipboard, variant)?;
                    println!(
                        "Clipboard content converted to {}",
                        if variant == Variant::ZhTW {
                            "Traditional Chinese (zh-TW)"
                        } else {
                            "Simplified Chinese (zh-CN)"
                        }
                    );
                }
            }
        }
        "paste" => {
            let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
            paste_clipboard(&mut clipboard)?;
        }
        "clean" => {
            let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
            clear_clipboard(&mut clipboard)?;
            println!("Clipboard cleared");
        }
        filepath => {
            let mut clipboard = Clipboard::new().context("Failed to initialize clipboard")?;
            copy_source_to_clipboard(&mut clipboard, filepath, None)?;
            println!("File content copied to clipboard: {}", filepath);
        }
    }

    Ok(())
}

/// Safely convert file in-place with automatic backup
fn convert_file_in_place(filepath: &str, variant: Variant, show_preview: bool) -> Result<()> {
    let path = Path::new(filepath);

    // 1. Read original content
    let original_content =
        fs::read_to_string(path).with_context(|| format!("Failed to read file: {}", filepath))?;

    // 2. Convert content
    let converted_content = zhconv(&original_content, variant);

    // 3. Create backup path (file.txt → file.txt.bak)
    let backup_path = append_backup_suffix(path)?;

    // 4. Write backup BEFORE modifying original
    fs::write(&backup_path, &original_content)
        .with_context(|| format!("Failed to create backup: {}", backup_path.display()))?;

    // 5. Show preview (unless forced)
    if show_preview {
        print_conversion_preview(&original_content, &converted_content, filepath);
        print_confirmation_prompt()?;
    }

    // 6. Write converted content back to original file
    fs::write(path, &converted_content)
        .with_context(|| format!("Failed to write converted content to: {}", filepath))?;

    println!(
        "{} Backup saved to: {}",
        "ℹ".yellow(),
        backup_path.display().to_string().dimmed()
    );

    Ok(())
}

/// Append .bak suffix preserving full filename (file.txt → file.txt.bak)
fn append_backup_suffix(path: &Path) -> Result<PathBuf> {
    let mut backup_path = path
        .as_os_str()
        .to_os_string()
        .into_string()
        .map_err(|_| anyhow::anyhow!("Invalid filename encoding"))?;
    backup_path.push_str(BACKUP_SUFFIX);
    Ok(PathBuf::from(backup_path))
}

/// Show conversion preview with character count
fn print_conversion_preview(original: &str, converted: &str, filepath: &str) {
    const MAX_PREVIEW_CHARS: usize = 120;

    let truncate = |s: &str| -> String {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() <= MAX_PREVIEW_CHARS {
            return s.to_string();
        }
        let truncated: String = chars.iter().take(MAX_PREVIEW_CHARS).collect();
        format!("{}{}", truncated, "...".dimmed())
    };

    println!("\n{} Converting: {}", "🔄".cyan(), filepath.bold());
    println!("{}", "─".repeat(60).dimmed());
    println!(
        "{} Original ({} chars):",
        "◂".blue(),
        original.chars().count()
    );
    println!("{}", truncate(original).bright_black());
    println!(
        "\n{} Converted ({} chars):",
        "▸".green(),
        converted.chars().count()
    );
    println!("{}", truncate(converted));
    println!("{}", "─".repeat(60).dimmed());
}

/// Simple confirmation prompt (press Enter to continue, Ctrl+C to abort)
fn print_confirmation_prompt() -> Result<()> {
    println!(
        "\n{} Press {} to apply changes, or {} to abort",
        "❓".yellow(),
        "Enter".green(),
        "Ctrl+C".red()
    );

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;

    Ok(())
}

// --- Existing helper functions (unchanged but included for completeness) ---

/// Launch fzf with list of files and return selected path
fn select_file_with_fzf(files: &[String]) -> Result<String> {
    // Prepare input for fzf (newline-separated)
    let input = files.join("\n");

    // Build fzf command with nice UX defaults
    let mut cmd = Command::new("fzf")
        .args([
            "--height=40%",
            "--layout=reverse",
            "--border",
            "--prompt=🎵 Select lyric file › ",
            "--header",
            &format!("{} files found", files.len()),
            "--preview",
            "head -n 20 {} | bat --color=always --style=plain --language=txt 2>/dev/null || head -n 20 {}",
            "--preview-window",
            "right:50%:wrap",
            "--bind",
            "ctrl-d:preview-down,ctrl-u:preview-up",
            "--exit-0", // exit 0 even when no selection (we handle empty output)
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to launch fzf")?;

    // Write file list to fzf's stdin
    if let Some(mut stdin) = cmd.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(input.as_bytes())
            .context("Failed to send data to fzf")?;
    }

    // Read selected file from fzf's stdout
    let output = cmd.wait_with_output().context("fzf process failed")?;

    if output.status.success() {
        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(selected)
    } else {
        Ok(String::new()) // Empty = cancelled
    }
}

fn copy_source_to_clipboard(
    clipboard: &mut Clipboard,
    filepath: &str,
    _preview_label: Option<&str>, // Unused in this version
) -> Result<()> {
    let content = read_source(filepath)?;
    clipboard
        .set_text(content)
        .context("Failed to set clipboard content")?;
    Ok(())
}

fn convert_source_to_clipboard(
    clipboard: &mut Clipboard,
    source: &str,
    variant: Variant,
) -> Result<()> {
    let content = read_source(source)?;
    let converted = zhconv(&content, variant);
    print_preview("Conversion preview", &converted);
    clipboard
        .set_text(converted)
        .context("Failed to set clipboard content")?;
    Ok(())
}

fn convert_clipboard(clipboard: &mut Clipboard, variant: Variant) -> Result<()> {
    let content = clipboard
        .get_text()
        .context("Failed to get clipboard content")?;
    let converted = zhconv(&content, variant);
    print_preview("Conversion preview", &converted);
    clipboard
        .set_text(converted)
        .context("Failed to set clipboard content")?;
    Ok(())
}

fn paste_clipboard(clipboard: &mut Clipboard) -> Result<()> {
    let content = clipboard
        .get_text()
        .context("Failed to get clipboard content")?;
    io::stdout()
        .write_all(content.as_bytes())
        .context("Failed to write to stdout")?;
    if !content.ends_with('\n') {
        io::stdout().write_all(b"\n")?;
    }
    Ok(())
}

fn clear_clipboard(clipboard: &mut Clipboard) -> Result<()> {
    clipboard
        .set_text("")
        .context("Failed to clear clipboard")?;
    Ok(())
}

fn read_source(source: &str) -> Result<String> {
    if source == "-" {
        let mut buffer = String::new();
        io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        Ok(buffer)
    } else {
        fs::read_to_string(source).with_context(|| format!("Failed to read file: {}", source))
    }
}

/// Show preview of log sanitization
fn print_log_sanitization_preview(original: &str, sanitized: &str, target: &str, count: usize) {
    const MAX_PREVIEW_LINES: usize = 12;

    let truncate_lines = |content: &str| -> String {
        let lines: Vec<&str> = content.lines().take(MAX_PREVIEW_LINES).collect();
        let mut result = lines.join("\n");
        if content.lines().count() > MAX_PREVIEW_LINES {
            result.push_str(&format!("\n{}...", "...".dimmed()));
        }
        result
    };

    println!(
        "\n{} Sanitizing log: replacing {} → {}",
        "🧹".cyan(),
        target.bold(),
        "root".bold().red()
    );
    println!("{}", "─".repeat(60).dimmed());

    println!("{}", "Before:".blue().bold());
    println!("{}", truncate_lines(original).bright_black());

    println!("\n{}", "After:".green().bold());
    println!("{}", truncate_lines(sanitized));

    println!("{}", "─".repeat(60).dimmed());
    println!(
        "{} {} occurrence{} replaced",
        "ℹ".yellow(),
        count,
        if count == 1 { "" } else { "s" }
    );
}

fn print_preview(label: &str, content: &str) {
    const MAX_PREVIEW_CHARS: usize = 100;
    let preview = if content.chars().count() > MAX_PREVIEW_CHARS {
        content.chars().take(MAX_PREVIEW_CHARS).collect::<String>() + "..."
    } else {
        content.to_string()
    };
    println!("\n--- {} ---", label.bold());
    println!("{}", preview.dimmed());
    println!(
        "{} characters\n",
        content.chars().count().to_string().dimmed()
    );
}

fn print_usage() {
    const COMMAND_WIDTH: usize = 36;

    eprintln!(
        "{}",
        "pb - Clipboard utility with Chinese conversion & lyric workflows".bold()
    );
    eprintln!();
    eprintln!("{}", "Usage:".bold());
    print_aligned("pb [file]", "Copy file/stdin to clipboard", COMMAND_WIDTH);
    print_aligned(
        "pb log [TEXT] [-f]",
        "Sanitize stdin: replace TEXT (or $USER) → root → clipboard",
        COMMAND_WIDTH,
    );
    print_aligned(
        "pb tw [file]",
        "Convert → Traditional Chinese → clipboard",
        COMMAND_WIDTH,
    );
    print_aligned(
        "pb cn [file]",
        "Convert → Simplified Chinese → clipboard",
        COMMAND_WIDTH,
    );
    print_aligned(
        "pb tw -i file",
        "In-place convert to Traditional Chinese (with .bak)",
        COMMAND_WIDTH,
    );
    print_aligned(
        "pb cn -i file",
        "In-place convert to Simplified Chinese (with .bak)",
        COMMAND_WIDTH,
    );
    print_aligned(
        "pb paste",
        "Output clipboard content to stdout",
        COMMAND_WIDTH,
    );
    print_aligned("pb clean", "Clear clipboard content", COMMAND_WIDTH);
    print_aligned(
        "pb lrc [opts]",
        "Interactive lyric (.lrc) file selection",
        COMMAND_WIDTH,
    );

    eprintln!();
    eprintln!("{}", "Options:".bold());
    print_aligned(
        "-i, --in-place",
        "Edit file in-place (creates .bak backup)",
        COMMAND_WIDTH,
    );
    print_aligned(
        "-f, --force",
        "Skip preview prompt for in-place edits",
        COMMAND_WIDTH,
    );
    print_aligned("-h, --help", "Show this help message", COMMAND_WIDTH);
    print_aligned("-V, --version", "Show version information", COMMAND_WIDTH);

    eprintln!();
    eprintln!("{}", "LRC Selection Commands:".bold());
    print_aligned(
        "pb lrc",
        "Select .lrc file → copy to clipboard",
        COMMAND_WIDTH,
    );
    print_aligned(
        "pb lrc tw",
        "Select → convert to Traditional Chinese → clipboard",
        COMMAND_WIDTH,
    );
    print_aligned(
        "pb lrc cn",
        "Select → convert to Simplified Chinese → clipboard",
        COMMAND_WIDTH,
    );
    print_aligned(
        "pb lrc -i tw",
        "Select → in-place convert to Traditional Chinese",
        COMMAND_WIDTH,
    );
    print_aligned(
        "pb lrc -r",
        "Recursive search in subdirectories",
        COMMAND_WIDTH,
    );
    print_aligned(
        "pb lrc -p PATTERN",
        "Filter files (e.g., '*love*', '*.txt')",
        COMMAND_WIDTH,
    );

    /*
    eprintln!();
    eprintln!("{}", "Examples:".bold());

    print_aligned_example("pb lyrics.txt", "Copy file to clipboard");
    print_aligned_example("cat file | pb", "Pipe stdin to clipboard");
    print_aligned_example("pb tw", "Convert clipboard → Traditional Chinese");
    print_aligned_example(
        "pb cn poem.txt",
        "Convert file → Simplified Chinese → clipboard",
    );
    print_aligned_example("pb tw -i song.lrc", "In-place convert with .bak backup");
    print_aligned_example("pb tw -i -f batch/\*.txt", "Force in-place edit (no prompt)");
    print_aligned_example("pb paste | grep chorus", "Search clipboard content");
    print_aligned_example("pb paste > out.txt", "Save clipboard to file");

    print_aligned_example(
        "cargo build 2>&1 | pb log",
        "Sanitize build logs before sharing",
    );
    print_aligned_example(
        "pb log -f < error.log",
        "Force sanitize without preview prompt",
    );
    eprintln!();
    eprintln!("{}", "Lyric Workflows:".bold().cyan());
    print_aligned_example("pb lrc", "Interactive .lrc selection → clipboard");
    print_aligned_example("pb lrc tw", "Select → convert to Traditional Chinese");
    print_aligned_example("pb lrc -i cn", "In-place Simplified Chinese conversion");
    print_aligned_example(
        "pb lrc -r -p '*live*'",
        "Recursive search for live versions",
    );
    print_aligned_example("pb lrc | pb paste | less", "Select → view converted lyrics");
    print_aligned_example(
        "cp song.lrc.bak song.lrc",
        "Recover from auto-generated backup",
    );
    */
    eprintln!();
    eprintln!(
        "{} {}",
        "💡 Tip:".yellow().bold(),
        "Combine with fzf/bat for powerful lyric workflows (install via brew/apt)".dimmed()
    );
}

fn print_aligned(command: &str, description: &str, width: usize) {
    use colored::Colorize;
    eprintln!(
        "  {:<width$}{}",
        command.cyan(),
        description.bright_black(),
        width = width
    );
}

fn print_aligned_example(command: &str, comment: &str) {
    use colored::Colorize;
    eprintln!(
        "  {:<36}{}",
        command.green(),
        format!("# {}", comment).bright_black()
    );
}
