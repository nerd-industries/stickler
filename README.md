# Stickler

A Linux label app for the **Nelko P21** Bluetooth label maker, written in Rust. It comes with an MCP server, so Claude can print labels for you too.

- 🏷️ **Template gallery with a live preview.** Text, icon + text, QR codes, barcodes, date stamps and images. Save your own templates as well.
- 📏 **Knows your labels.** The app reads the loaded roll's size from the printer, so you never have to pick it.
- 🤖 **MCP server.** Claude can preview and print labels (`stickler-mcp`).
- 🐧 **Plain Bluetooth through BlueZ.** No root, no `rfcomm bind`, no phone.

## Install

Needs Rust, Node.js, BlueZ, and the Tauri prerequisites for your distro (on Arch: `webkit2gtk-4.1 base-devel openssl librsvg`), plus `cargo install tauri-cli --version "^2"`.

```sh
git clone https://github.com/nerd-industries/stickler && cd stickler
packaging/install.sh
```

This installs `stickler` and `stickler-mcp` into `~/.local/bin`, adds **Stickler** to your app menu, and registers the MCP server with Claude Code if `claude` is installed.

## First run

1. Turn the P21 on. If the phone app is connected to it, close the app, because the printer only takes one connection at a time.
2. Open Stickler, click the printer box at the bottom left, choose **Search**, then **Pair**. No PIN is needed.
3. Pick a template, type your text, then press **Print** (or Ctrl+Enter).

## Using it from Claude

Once the server is registered (`claude mcp add --scope user stickler -- ~/.local/bin/stickler-mcp`), ask things like *"print a label for the leftovers dated today"* or *"make a Wi-Fi QR label for my guest network"*.

| Tool | What it does |
|---|---|
| `printer_status` | Connection, battery, status, and the loaded roll |
| `list_templates` | Templates and their fields, plus the icon names |
| `preview_label` | Renders a PNG preview without printing |
| `print_label` | Prints a template with your values (1–10 copies) |
| `print_text` | Quick text label |
| `print_image` | Prints a local image, dithered to black and white |
| `save_template` | Saves a reusable template |

## Troubleshooting

- **"Printer is off, out of range, or connected to another device"**: switch it on and close the Nelko phone app.
- **"The lid is open"** or **"Out of labels"**: these come straight from the printer.
- **Blank window on Wayland**: Stickler runs through XWayland by default for this reason. Set `GDK_BACKEND=wayland` to try native Wayland.

## Development

```sh
cargo test --workspace                          # protocol, rendering, templates, service, MCP, app commands
cargo run -p nelko-core --example probe -- info  # talk to the printer from the terminal
cd app/src-tauri && cargo tauri dev             # run the app with hot reload
```

Layout: `crates/core` (protocol, Bluetooth, rendering, templates, the `Nelko` service), `crates/mcp` (MCP server), `app/` (Tauri v2 + Svelte UI). The design and protocol notes are in `docs/superpowers/specs/`.

## Credits

- Protocol details: [merlinschumacher/nelko-p21-print](https://github.com/merlinschumacher/nelko-p21-print), plus our own testing on real hardware. No code is copied from it.
- Icons: [Lucide](https://lucide.dev) (ISC). Fonts: Roboto, Roboto Condensed, Nunito and Inter (SIL OFL).

Not affiliated with Nelko. "Nelko" and "P21" are used only to say which printer this works with.

## License

MIT
