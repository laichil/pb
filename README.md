# pb - Clipboard Utility with Chinese Conversion

A powerful clipboard utility with built-in Chinese text conversion (Simplified/Traditional), log sanitization, and interactive lyric file workflows.

## Features

- 📋 Copy files or stdin to clipboard
- 🔄 Convert between Simplified and Traditional Chinese
- 🧹 Sanitize logs (replace usernames/paths with "root")
- 🎵 Interactive lyric file selection with preview
- 📝 In-place file conversion with automatic backups
- ⚡ FZF integration for fuzzy file selection

## Installation

### From Source
```bash
cargo install pb-clipboard
```

### Build from Repository
```bash
git clone https://github.com/yourusername/pb
cd pb
cargo build --release
sudo cp target/release/pb /usr/local/bin/
```

## Usage

### Basic Commands

```bash
# Copy file to clipboard
pb file.txt

# Copy stdin to clipboard
cat file.txt | pb
echo "Hello" | pb

# Copy clipboard to stdout
pb paste

# Clear clipboard
pb clean

# Show help
pb --help

# Show version
pb --version
```

### Chinese Conversion

```bash
# Convert clipboard to Traditional Chinese
pb tw

# Convert clipboard to Simplified Chinese
pb cn

# Convert file to Traditional Chinese
pb tw file.txt

# Convert file to Simplified Chinese
pb cn file.txt

# In-place conversion (creates .bak backup)
pb tw -i file.txt
pb cn -i file.txt

# Force in-place (skip preview)
pb tw -i -f file.txt
```

### Log Sanitization

```bash
# Auto-detect username and sanitize
cargo build 2>&1 | pb log

# Replace specific text
pb log "myusername" < log.txt

# Skip preview prompt
pb log -f < log.txt
```

### Lyric File Workflows (LRC)

```bash
# Interactively select .lrc file and copy to clipboard
pb lrc

# Select and convert to Traditional Chinese
pb lrc tw

# Select and convert to Simplified Chinese
pb lrc cn

# In-place conversion with backup
pb lrc -i tw
pb lrc -i cn

# Force in-place conversion (skip preview)
pb lrc -i -f tw

# Recursive search
pb lrc -r

# Custom file pattern
pb lrc -p "*.txt"
pb lrc -r -p "*live*"
```

## Examples

### Common Use Cases

```bash
# Copy a file to clipboard
pb notes.txt

# Copy build logs (sanitized)
cargo build 2>&1 | pb log

# Convert Chinese text
pb tw "Hello 世界"  # Converts to Traditional Chinese

# Convert file in-place
pb cn -i document.txt  # Creates document.txt.bak

# Select a lyric file and convert to Traditional Chinese
pb lrc tw

# Search for lyrics in subdirectories
pb lrc -r -p "*2024*"

# Save clipboard content
pb paste > output.txt

# Search clipboard content
pb paste | grep "pattern"
```

### Advanced Workflows

```bash
# Convert clipboard to Traditional Chinese and save
pb tw && pb paste > converted.txt

# Batch convert all .lrc files to Simplified Chinese (with preview)
for file in *.lrc; do pb cn -i "$file"; done

# Force batch conversion (no preview)
for file in *.lrc; do pb cn -i -f "$file"; done

# Interactive file selection with preview
pb lrc -r -p "*.txt" | xargs -I {} pb tw {}

# Copy sanitized log and convert to Traditional Chinese
cargo build 2>&1 | pb log | pb tw
```

## Features in Detail

### Chinese Conversion
- Uses the `zhconv` library for accurate conversion
- Supports both Simplified (zh-CN) and Traditional (zh-TW)
- Preserves non-Chinese text and formatting
- Automatic backup (.bak) for in-place conversions

### Log Sanitization
- Replaces usernames or specific text with "root"
- Uses word boundaries to avoid partial replacements
- Shows preview before applying changes
- Useful for sharing logs while protecting sensitive information

### Lyric File Workflows
- Interactive selection using FZF
- Preview first 20 lines with syntax highlighting (if `bat` installed)
- Support for recursive search
- In-place conversion with automatic backup

## Dependencies

- **Required**: `fzf` - For interactive file selection
- **Optional**: `bat` - For better preview syntax highlighting

### Install Dependencies

```bash
# macOS
brew install fzf bat

# Ubuntu/Debian
sudo apt install fzf bat

# Arch Linux
sudo pacman -S fzf bat

# Windows (via Chocolatey)
choco install fzf bat
```

## Configuration

### Environment Variables
- `USER` or `USERNAME` - Auto-detected for log sanitization
- `EDITOR` - Used for file editing (not currently used)

### Backup Files
- In-place conversions create backups with `.bak` suffix
- Example: `file.txt` → `file.txt.bak`
- Backup is created BEFORE modification

## Keyboard Shortcuts (FZF)

When using `pb lrc`:
- `Ctrl+U` / `Ctrl+D` - Scroll preview up/down
- `Enter` - Select file
- `Esc` or `Ctrl+C` - Cancel selection

## License

MIT License

## Contributing

1. Fork the repository
2. Create a feature branch
3. Commit your changes
4. Push to the branch
5. Open a Pull Request

## Troubleshooting

### FZF Not Found
```bash
# Install fzf (see dependencies section)
# Or use the default file selection behavior
```

### Permission Denied
```bash
# Ensure the binary is executable
chmod +x target/release/pb
```

### Clipboard Issues (Linux)
```bash
# For X11
sudo apt install xclip xsel

# For Wayland
sudo apt install wl-clipboard
```

## Acknowledgments

- [zhconv](https://crates.io/crates/zhconv) - Chinese conversion library
- [fzf](https://github.com/junegunn/fzf) - Fuzzy finder
- [arboard](https://crates.io/crates/arboard) - Clipboard library

---

**Tip**: Combine with `bat`, `fzf`, and `grep` for powerful text processing workflows!
