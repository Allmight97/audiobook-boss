# Tauri + Vanilla TS

This template should help get you started developing with Tauri in vanilla HTML, CSS and Typescript.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Logging Configuration

The application uses the `RUST_LOG` environment variable to control logging output. Logs are written to stderr and can be configured at different verbosity levels.

### Setting Log Levels

You can control the logging output by setting the `RUST_LOG` environment variable before running the application:

#### Verbose Output (Debug Level)
```bash
RUST_LOG=debug npm run tauri dev
```
This shows all debug, info, warn, and error messages. Useful for development and troubleshooting.

#### App-Specific Debug
```bash
RUST_LOG=audiobook_boss=debug npm run tauri dev
```
This shows debug messages only from the audiobook_boss application, filtering out logs from dependencies.

#### Standard Logging (Info Level)
```bash
RUST_LOG=info npm run tauri dev
```
This shows informational messages, warnings, and errors. Good for general operation monitoring.

#### Warnings Only
```bash
RUST_LOG=warn npm run tauri dev
```
This shows only warnings and errors, minimizing output for production use.

### Log Output

- **Location**: All logs are written to stderr (standard error stream)
- **Format**: Logs include timestamp, level, module path, and message
- **Example output**:
  ```
  [2024-01-20T10:30:45Z DEBUG audiobook_boss::audio] Processing audio file: example.mp3
  [2024-01-20T10:30:46Z INFO  audiobook_boss::commands] Conversion completed successfully
  ```

### Advanced Configuration

You can also combine multiple log level specifications:

```bash
# Debug for app, warn for everything else
RUST_LOG=warn,audiobook_boss=debug npm run tauri dev

# Trace level for specific module
RUST_LOG=audiobook_boss::audio=trace npm run tauri dev
```

For production builds, you can set the environment variable when running the built application:

```bash
RUST_LOG=info ./target/release/audiobook-boss
```
