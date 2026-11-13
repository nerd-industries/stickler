# Nelko P21 for Linux: Design

Date: 2026-11-12
Status: Draft, awaiting approval

## 1. Intent

The user owns a Nelko P21 Bluetooth label maker. The only official software is an Android/iOS app. They want:

- A **good-looking Linux desktop app** (KDE Plasma, Wayland, CachyOS) that does what the phone app does: pick a template, type the text, see a preview, print. It should be easy to use.
- **No need to know the label size.** The user forgot what rolls they have. The app should find out from the printer.
- An **MCP server** so Claude can print labels for them (for example, "print a label for the leftovers dated today").

Written in **Rust**. Linux only.

## 2. Goals, non-goals, anti-goals

**Goals**
- Print over Bluetooth Classic (RFCOMM) from Linux, without root.
- Read the loaded roll's size, gap and type from the printer (the roll has an NFC tag the printer reads).
- A template gallery: text, icon + text, QR + text, barcode, borders, date stamp, image, and the user's own saved templates.
- A live preview that matches the printed label exactly.
- An MCP server that shares templates, settings and rendering with the app.
- Show the printer's battery and status (lid open, no paper, and so on).

**Non-goals (v1)**
- Windows and macOS.
- BLE transport (documented, but Classic works on Linux and is simpler). Possibly later.
- Other printer models. The P22 reportedly uses the same protocol. It isn't a target, but don't hard-code anything that blocks it.
- Nelko cloud templates, accounts, or login.
- Copying Nelko's template artwork, fonts or icons. We make our own templates.
- A free-form drag-and-drop canvas editor. Templates have fixed layouts with editable fields and options.

**Anti-goals (failure even if it "works")**
- The preview doesn't match the print (wrong orientation, clipping, wrong scale).
- The app or MCP server holds the Bluetooth link open and locks out the other one. The printer allows **only one connection at a time**.
- It needs root, or the user has to run `rfcomm bind` by hand.
- It looks like a developer tool rather than a polished app.

## 3. Prior art and protocol

The protocol has already been reverse engineered from a Bluetooth capture of the official app:

- https://github.com/merlinschumacher/nelko-p21-print (protocol doc and Python script, **AGPL-3.0**)
- https://github.com/merlinschumacher/nelko-p21-capture (raw Wireshark capture)
- https://github.com/kennedn/nelko-p21 (Python CLI)

**License rule:** implement from the protocol description (facts). Do **not** copy code from the AGPL repos.

### 3.1 Transport
- Bluetooth Classic SPP / RFCOMM, **channel 1**. One connection at a time.
- Text commands are ASCII, end in `\r\n`, and are case-insensitive. Query replies end in `\r\n`.
- Status commands are raw `ESC` sequences with raw-byte replies and no terminator.

### 3.2 Print job (no replies)
```
SIZE 14 mm,40 mm\r\n
GAP 5 mm,0 mm\r\n
DIRECTION 1,1\r\n
DENSITY 15\r\n            (1–15)
CLS\r\n
BITMAP 0,0,12,320,1,<wbytes*h raw bytes>
PRINT 1\r\n               (copies)
```
- Bitmap: 1 bit per pixel, MSB first, rows padded to whole bytes, **0 bit = black**. Mode 1 = OR.
- The print head is **96 dots wide (12 mm) at 203 DPI**. Wider bitmaps are clipped (rightmost 96 dots kept). Bitmap height = label length in dots (40 mm → 320).
- **Unknown:** whether a `\r\n` follows the raw bitmap bytes. Confirm from the capture (`nelko_bt_capture.pcapng`) before relying on either.
- `BEEP <b>` and `TIMER <b>` take a raw byte argument, not ASCII digits.
- Also present: `SELFTEST`. `INITIALPRINTER` is a factory reset. **Never expose it.**

### 3.3 Queries
- `BATTERY?` → `BATTERY` + 2 bytes + CRLF. Byte 0 is the charge in BCD (`0x99` = 99%; it reads 99 while charging). Byte 1 = 1 if charging.
- `CONFIG?` → `CONFIG` + 10 bytes (a 20-byte variant exists; the first 10 match): DPI u16 BE, hardware version (3 bytes), firmware version (3 bytes), shutdown timer (0 never / 1 = 15 min / 2 = 30 / 3 = 60), beep 0/1.
- `ESC ! ?` (`1B 21 3F`) → 1 status byte: 00 ready, 01 lid open, 02 feeding, 03 paper error, 04 no paper, 10 paused, 20 busy, 24 out of paper, 30 low battery, 80 overheated, 90 no power. Only 00 and 04 have been observed. Unknown values show as "Unknown status 0xNN".
- `ESC ! o` (`1B 21 6F`) → 16-byte label block:

| Offset | Field |
|---|---|
| 0 | status byte |
| 1 | tag length (0x0C) |
| 2–13 | 12-byte paper code (all zeros = no readable tag) |
| 3 | high nibble: tag format version; low nibble: trade flag |
| 4 | colour: 2 transparent, 3 white, 4 pink, 5 blue, 6 yellow |
| 5–6 | batch number (BE) |
| 7 | paper type: 0 continuous, 1 gapped, 2 black mark |
| 8 | gap in mm (format ≥ 2) |
| 9, 10 | roll outer size, roll length (units unknown: display raw, don't rely on them) |
| 11 | label length mm |
| 12 | usable width (unit unclear; example 15) |
| 13 | label width mm |
| 14–15 | CRC16/MODBUS over bytes 0–13, big-endian |

Example: `00 0c 01 12 03 00 03 01 12 12 15 28 0f 0e ed 03` → a 14 × 40 mm white gapped roll. **Use it as a golden test vector, CRC included** (the reviewer verified that CRC16/MODBUS over bytes 0–13 = `0xED03`, stored BE).

### 3.4 Confirmed from the official APK (decompiled with jadx, 2026-11-12)
The source is in `~/nelko-re/apk-src` (outside the repo, never committed). Facts only; no code copied.

**Tag parsing** (`com.ezink.data.ConsumablesPaperBean.parse`). This supersedes the table above where they differ:
- Valid only if the length is 16, `b[1] == 0x0C`, the CRC is OK, and bytes 2..13 aren't all zero. Bytes 11..13 must also not all be zero.
- Common fields: colour = b[4], batch = b[5..6], paper type = b[7], length mm = b[11].
- If **b[3] == 0x02** (legacy format): width = b[12].
- Otherwise width = b[13], and the high nibble of b[3] is the *encoding version*:
  - **v1** (e.g. `0x12`, which is the P21 roll): **there's no gap on the tag**. b[8] is *not* the gap, so the example's "18 mm" was a misreading.
  - **v2/v3**: gap mm = b[8], and paper type = the low nibble of b[7].
- **Gap fallback when the tag has none: 5 mm** (the P21 `defaultGap`). Continuous paper sends `GAP 0 mm,0 mm`.

**P21 device profile** (`assets/release/cloud_config_nelko.json`): maxWidth 12 mm, 203 dpi, label length 10–75 mm, default paper 14×40 gapped white, defaultGap 5, `isCompression 0` (plain uncompressed BITMAP, no QuickLZ), max 50 copies, default rotation 0. Its default paper code `011203000301121215280F0E` is exactly the example tag above.

**Printable length** (`Devices.getEffectivePrintValue`). The image is shorter than the label and offset down the feed axis by `(label − printable) / 2`:
- 40 mm label → 35.5 mm printed (offset 2.25 mm). 30 mm label → 25 mm (offset 2.5 mm).
- Any other length → length − 2 mm (offset 1 mm).
So the bitmap height is the printable length × 8 dots, and the `BITMAP` y = offset × 8.

**Job sequence** (non-compressed path): `SIZE <w> mm,<len> mm` → `GAP <g> mm,0 mm` → `DIRECTION 1,1`-style forward/normal → `DENSITY n` → `CLS` → `BITMAP x,y,wbytes,h,mode,` + raw bytes → `PRINT n`. The app formats Java floats (`SIZE 14.0 mm,40.0 mm`); the captured output uses integers. Either is presumably accepted, so the builder should match the capture.

**Known label sizes for P21** (`LabelPaperBean`) with paper codes `CC TT LL WW SS` (colour, type, length, width, shape):
12×40, 14×30, 14×40, 14×50, 14×75, 15×22, 15×30, 15×40, 15×50, 15×75. Colours seen are white, transparent, pink, blue and yellow, depending on size. Use these as the manual-pick presets.

**"DRM"**: the app only shows a mismatch dialog if the tag size ≠ the selected template size. The printer doesn't enforce anything.

## 4. Approaches considered

**Desktop UI (user picked Tauri):**
1. **Tauri v2**, web frontend plus Rust backend: picked. It's the easiest way to a polished look, and the Rust core is shared directly.
2. Slint: native Rust and lighter, but harder to make look as polished.
3. egui: fastest to build, but looks like a dev tool.
GTK/libadwaita was rejected because it looks foreign on KDE.

**How "just print" was decided:** the protocol is already documented, so the work is building on it. Decompiling the APK is optional and only helps decode the unknown roll fields (§9).

## 5. Architecture

One Cargo workspace, three crates:

```
nelko/
  crates/core/     nelko-core: protocol, transport, label info, templates, rendering, config
  crates/mcp/      nelko-mcp: MCP server binary (stdio)
  app/             Tauri v2 app (src-tauri depends on nelko-core; frontend in app/ui)
```

Both front ends are thin. All behaviour lives in `nelko-core`, so the app and Claude print exactly the same thing.

### 5.1 nelko-core
- **protocol**: encode commands to bytes, decode replies (battery, config, status, label block, CRC16/MODBUS). Pure functions, fully unit-tested.
- **transport**: a `Transport` trait (write, read with timeout). The real implementation is an RFCOMM socket via the `bluer` crate (BlueZ D-Bus), so it needs no `rfcomm bind` and no root. A mock implementation records bytes for tests.
- **printer session**: connect, optionally query status/label/battery, send the job, then **disconnect right away**. Before connecting, take an exclusive cross-process lock (`flock` on `$XDG_RUNTIME_DIR/nelko-p21.lock`) so the app and the MCP server never collide. If the lock is held, wait up to about 10 s, then give a clear "printer busy" error.
- **discovery/pairing**: scan for devices whose name starts with `P21` (confirm the advertised name on real hardware), pair and trust via BlueZ, and save the MAC address in config.
- **templates**: data model, built-in templates, and user templates (below).
- **render**: template + field values + label size → a 1-bit bitmap plus a preview PNG, both from the same pixels.
- **config**: `~/.config/nelko/config.toml` holds the printer MAC, default density, last known label size and manual fallback size. User templates go in `~/.config/nelko/templates/*.json`.

### 5.2 Orientation and geometry (critical)
- The head runs across the tape (12 mm, 96 dots). The label **length** runs along the feed direction.
- Templates are laid out **landscape** (label length × label width, for example 40 × 14 mm, reading along the tape, the way the phone app shows it). The renderer then rotates by 90° to make the 96-dot-wide print bitmap. Pick the rotation direction so that text reads correctly as the label comes out of the printer. **Verify it on real hardware with an asymmetric test print**, and put the result in a test.
- Printable area: the width is `min(label width, 12 mm)` = 96 dots. The length and offset follow §3.4 exactly (for example, 40 mm → 284 dots tall at y = 18). Templates lay out inside this printable rectangle, not the full label.
- The preview shows the label shape, rounded corners and the non-printable margin, at the same pixel grid as the print, scaled up with nearest-neighbour.

### 5.3 Label size detection
Order of preference:
1. The `ESC ! o` block with a valid CRC and a non-zero paper code: use its size, gap and type, and show the colour.
2. Otherwise: the last known size from config.
3. Otherwise: ask the user to pick from the §3.4 presets, or enter a custom size (length 10–75 mm).
The gap comes from the tag (v2+) or falls back to 5 mm.
The app shows the detected roll ("White 14 × 40 mm, gapped") in the header and refreshes it on connect and on a refresh button. For continuous rolls, the label length comes from the template or content (user-adjustable).

### 5.4 Templates
A template is JSON: `id`, `name`, `category`, `layout` kind, `fields` (named, typed slots: text, multiline text, date, QR data, barcode data, image path, icon name) with defaults, and `style` options (font, bold, alignment, border style, invert). Layout kinds:

| Layout | Fields | Notes |
|---|---|---|
| `text` | 1–3 lines | Auto-fits the largest font size that fits; per-line alignment |
| `icon_text` | icon, text lines | Icon on the left at full height |
| `qr_text` | QR data, text | QR square at full height on the left |
| `barcode` | data, optional caption | Code128 by default; EAN-13 if the data is valid for it |
| `date_stamp` | label (e.g. "Opened"), date (default today), format | |
| `image` | image path, threshold or dither | Floyd–Steinberg dithering by default |

Any layout can add `border`: none, thin, thick, rounded or double. The built-in gallery ships about 15 to 20 ready-made templates across these layouts (for example "Kitchen jar", "Leftovers", "Cable tag", "Storage bin", "Wi-Fi QR", "Asset tag"), embedded in the binary. Duplicating any template creates a user template. Names are unique slugs, so Claude can refer to them.

### 5.5 Rendering stack (builder may swap equivalents)
- `tiny-skia` for drawing, and `cosmic-text` or `ab_glyph` for text.
- `resvg` for SVG icons. **Lucide** icons (ISC license) are bundled.
- `qrcode` for QR codes and `barcoders` for barcodes. Bars must land on whole dots so they scan.
- Bundled OFL fonts: Inter (regular and bold) plus one condensed and one rounded or display face. System fonts are optional through `fontdb`.
- Threshold to 1-bit **after** layout; text gets a crisp threshold, images get dithering.

### 5.6 Desktop app (Tauri v2)
- The frontend is Svelte + TypeScript + Vite (builder's call, kept light). Tauri commands call into `nelko-core`. **All label pixels come from Rust**, never from browser canvas text, so the preview is exactly what prints.
- Screens:
  - **Gallery**: template cards, each with a live thumbnail, grouped by category, with search.
  - **Editor**: big live preview on top, fields and style options below, then Copies, Density and **Print**. The preview updates as you type (debounced).
  - **Printer panel**: connection, battery, status, roll info, and "Find printer" pairing flow. Self-test lives under advanced.
- Look: modern, calm, polished. It follows system light/dark mode, uses smooth transitions and clear error states ("Lid is open", "Printer is off or out of range"). It should fit in on KDE but doesn't need to imitate Breeze.
- Installed as a native binary with a `.desktop` file and icon.

### 5.7 MCP server (nelko-mcp)
- Rust, the `rmcp` crate, stdio transport. Registered with `claude mcp add nelko -- <path>/nelko-mcp`.
- Tools:
  - `printer_status`: battery, status, loaded roll (size, type, colour) or "unknown".
  - `list_templates`: id, name, layout, fields and defaults.
  - `preview_label(template, fields, style?)`: returns a PNG image so Claude can see it before printing.
  - `print_label(template, fields, style?, copies=1, density?)`: renders and prints, then returns success or a clear error.
  - `print_text(text, …)`: shortcut for the auto-fit text template.
  - `print_image(path, …)`: prints a local image.
  - `save_template(...)`: optional in v1; lets Claude create a user template.
- Every print and preview tool takes an optional `label_size` (for example `"14x40"`). **Unknown size (my call):** use the tag size first, then the explicit `label_size`, then the last known size from config, and **mention the assumption in the result**. If none exists, refuse with an error telling Claude to ask the user or pass `label_size`. It never silently guesses.
- Safety: copies are capped (default max 10 per call), and factory reset or other config changes are never exposed.
- It uses the same config, templates and lock as the app.

## 6. Constraints
- Linux (KDE Plasma 6, Wayland), BlueZ. Rust 1.99 is installed. Tauri needs `webkit2gtk-4.1` and the usual build dependencies. The user has granted passwordless sudo for installing tooling.
- Single Bluetooth connection: connect per job and release right away.
- 203 DPI, 96-dot head width.

## 7. Testing
- Protocol: golden-byte tests for every command and for the example label block, CRC included.
- Rendering: snapshot tests, one PNG per built-in template.
- Session: mock transport that checks the exact byte sequence of a full job.
- Hardware smoke test, run by hand: an asymmetric orientation test, an edge-to-edge border to check clipping and margins, and a barcode or QR scan test.

## 8. Left to the builder
- Exact crate versions, frontend framework details, styling.
- Retry and timeout values, debounce timing, margin defaults.
- The exact built-in template list and artwork, within the layouts above.
- Dithering algorithm details.

## 9. Open questions (non-blocking)
- **Resolved:** the app appends `\r\n` after the raw bitmap bytes (`LabelCommand.initBitmap*`).
- **Partly resolved:** the app's FORWARD/NORMAL enum values are 0,0, so it would send `DIRECTION 0,0`, but the capture shows `DIRECTION 1,1`. Use the captured `1,1` (known to print). The hardware rotation test settles which one is visually correct.
- Density: use 1–15 with a default of 15 (what the capture used). The user can lower it in settings.

### 9.1 Hardware findings (user's P21, 2026-11-12) — binding
- Advertised name **`P21`**. Shows up in a BR/EDR scan; pairs with **no PIN**; exposes the Serial Port UUID.
- bluer's `rfcomm::Stream::connect` can return while the link is still coming up. Writes fail with ENOTCONN (107) for about 3 s, so the transport retries the first write until the connect deadline. (The earlier "no reply" was the missing CRLF, not timing.)
- **The status and label queries need a CRLF** on this firmware: `1B 21 3F 0D 0A` and `1B 21 6F 0D 0A`. Without it there's no reply.
- Replies have a **space after the keyword**: `BATTERY \x99\x00\r\n` (99%, not charging) and `CONFIG \x00\xcb\x00\x02\x02\x04\x02\x0e\x02\x01\r\n` (203 dpi, hw 0.2.2, fw 4.2.14, auto-off 30 min, beep on). Parsers accept an optional space.
- The roll tag is `000c011203000301121215280f0eed03`, identical to the documented example (14×40 white, gapped, v1).
- Status is `0x20` (busy) while printing and `0x00` when done (about 0.5 s per label at 14×40).
- **Rotation = clockwise**: landscape design → rotate 90° CW → rows of 96 dots. The label exits to the left with the printer held logo-up, start of the design first. The top of the design is the edge facing up. This was verified with an arrow test.
- The job `SIZE 14 mm,40 mm / GAP 5 mm,0 mm / DIRECTION 1,1 / DENSITY 15 / CLS / BITMAP 0,18,12,284,1,<data>\r\n / PRINT 2` printed 2 perfect labels: text crisp, border fully on the label, second copy aligned to the gap. **Density 15 confirmed good.**
