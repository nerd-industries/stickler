// Lucide SVGs shared with the Rust renderer (crates/core/assets/icons).
const raw = import.meta.glob('$icons/*.svg', { query: '?raw', import: 'default', eager: true }) as Record<string, string>
export const iconSvg: Record<string, string> = Object.fromEntries(
  Object.entries(raw).map(([path, svg]) => [path.split('/').pop()!.replace('.svg', ''), svg.replace(/<!--.*?-->/s, '')]),
)
