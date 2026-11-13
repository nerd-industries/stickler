// Typed wrappers over the Tauri commands in app/src-tauri/src/commands.rs.
import { invoke } from '@tauri-apps/api/core'

export type FieldKind = 'text' | 'multiline' | 'date' | 'qr_data' | 'barcode_data' | 'image_path' | 'icon'
export type Layout = 'text' | 'icon_text' | 'qr_text' | 'barcode' | 'date_stamp' | 'image'
export type FontChoice = 'sans' | 'sans_bold' | 'condensed' | 'rounded'
export type Align = 'left' | 'center' | 'right'
export type Border = 'none' | 'thin' | 'thick' | 'rounded' | 'double'

export interface FieldDef { key: string; label: string; kind: FieldKind; default: string }
export interface Style {
  font: FontChoice; bold: boolean; align: Align; border: Border; invert: boolean
  barcode: 'auto' | 'code128' | 'ean13'; date_format: string
}
export interface Template {
  id: string; name: string; category: string; layout: Layout; fields: FieldDef[]; style: Style; builtin: boolean
}
export type Values = Record<string, string>

export interface LabelSize { width_mm: number; length_mm: number }
export interface RollInfo {
  size: LabelSize; gap_mm: number | null; paper_type: string; color: string; encoding_version: number; paper_code: string
}
export interface PrinterReport {
  connected: boolean; name: string | null; status: { kind: string; code?: number } | null; status_text: string | null
  battery: { percent: number; charging: boolean } | null; roll: RollInfo | null; firmware: string | null; error: string | null
}
export interface Config {
  printer_mac: string | null; printer_name: string | null; density: number; rotation: 'cw' | 'ccw'
  last_label: LabelSize | null; manual_label: LabelSize | null
}
export interface FoundPrinter { mac: string; name: string; paired: boolean }
export interface Preview { png_base64: string; size: string; source: string }
export interface PrintOutcome { size: string; source: string; source_text: string }

/** Errors arrive as { message }; rethrow as Error so callers can show `.message`. */
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    const msg = typeof e === 'object' && e && 'message' in e ? String((e as { message: unknown }).message) : String(e)
    throw new Error(msg)
  }
}

export const api = {
  printerReport: () => call<PrinterReport>('printer_report'),
  discoverPrinters: () => call<FoundPrinter[]>('discover_printers'),
  pairPrinter: (mac: string, name?: string) => call<void>('pair_printer', { mac, name }),
  listTemplates: () => call<Template[]>('list_templates'),
  iconNames: () => call<string[]>('icon_names'),
  labelPresets: () => call<string[]>('label_presets'),
  preview: (templateId: string, values: Values, style: Style | null, size: string | null) =>
    call<Preview>('preview', { templateId, values, style, size }),
  print: (req: { template_id: string; values: Values; style: Style | null; size: string | null; copies: number; density: number | null }) =>
    call<PrintOutcome>('print', { req }),
  saveTemplate: (t: Template) => call<Template>('save_template', { t }),
  deleteTemplate: (id: string) => call<void>('delete_template', { id }),
  duplicateTemplate: (id: string, name: string) => call<Template>('duplicate_template', { id, name }),
  getSettings: () => call<Config>('get_settings'),
  setSettings: (c: Config) => call<Config>('set_settings', { c }),
  selfTest: () => call<void>('self_test'),
}

export const sizeLabel = (s: string) => s.replace('x', ' × ') + ' mm'
export const pngUrl = (b64: string) => `data:image/png;base64,${b64}`
