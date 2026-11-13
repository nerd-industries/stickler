# Stickler

A Linux label app for the **Nelko P21** Bluetooth label maker, written in Rust.

- 🏷️ Template gallery with a live preview (text, icons, QR codes, barcodes, date stamps, images)
- 📏 Reads the loaded roll's size from the printer automatically
- 🤖 MCP server so Claude can design and print labels for you
- 🐧 Plain Bluetooth Classic (RFCOMM) through BlueZ. No root, no `rfcomm bind`

> **Status:** early development. The protocol layer is done and verified on real hardware. The app and MCP server are in progress.

## Credits

The protocol details come from [merlinschumacher/nelko-p21-print](https://github.com/merlinschumacher/nelko-p21-print) plus our own testing on hardware. No code is copied from it.

Not affiliated with Nelko. "Nelko" and "P21" are used only to say which printer this works with.

## License

MIT
