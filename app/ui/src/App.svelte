<script lang="ts">
  import { onMount } from 'svelte'
  import { listen } from '@tauri-apps/api/event'
  import { api } from './lib/api'
  import type { PrinterReport, Template } from './lib/api'
  import TemplateList from './lib/TemplateList.svelte'
  import Editor from './lib/Editor.svelte'
  import PrinterPanel from './lib/PrinterPanel.svelte'

  let templates = $state<Template[]>([])
  let thumbs = $state<Record<string, string>>({})
  let selected = $state<Template | null>(null)
  let report = $state<PrinterReport | null>(null)
  let checking = $state(true)
  let panelOpen = $state(false)
  let presets = $state<string[]>([])
  let icons = $state<string[]>([])
  let density = $state(15)
  let toast = $state<{ msg: string; kind: 'ok' | 'error'; id: number } | null>(null)

  function notify(msg: string, kind: 'ok' | 'error' = 'ok') {
    const id = Date.now()
    toast = { msg, kind, id }
    setTimeout(() => {
      if (toast?.id === id) toast = null
    }, kind === 'error' ? 7000 : 3500)
  }

  async function loadTemplates(selectId?: string) {
    templates = await api.listTemplates()
    selected = templates.find((t) => t.id === (selectId ?? selected?.id)) ?? templates[0] ?? null
    // Thumbnails with each template's defaults; templates without defaults (image) stay blank.
    for (const t of templates) {
      if (thumbs[t.id]) continue
      api.preview(t.id, {}, null, null).then((p) => (thumbs[t.id] = p.png_base64)).catch(() => {})
    }
  }

  async function refresh() {
    checking = true
    try {
      report = await api.printerReport()
    } finally {
      checking = false
    }
  }

  onMount(() => {
    const unlisten = listen<PrinterReport>('printer-report', (e) => {
      report = e.payload
      checking = false
    })
    loadTemplates()
    api.labelPresets().then((p) => (presets = p))
    api.iconNames().then((n) => (icons = n))
    api.getSettings().then((c) => (density = c.density))
    return () => {
      unlisten.then((f) => f())
    }
  })

  const chip = $derived.by(() => {
    if (checking) return { tone: 'idle', title: 'Checking printer…', sub: '' }
    if (!report?.connected) return { tone: 'off', title: 'Printer not connected', sub: 'Click to set up' }
    const parts: string[] = []
    if (report.roll) parts.push(`${report.roll.size.width_mm} × ${report.roll.size.length_mm} mm labels`)
    if (report.battery) parts.push(`${report.battery.percent}% battery`)
    const ready = report.status?.kind === 'ready'
    return {
      tone: ready ? 'on' : 'warn',
      title: ready ? `${report.name ?? 'P21'} ready` : (report.status_text ?? 'Check printer'),
      sub: parts.join(', '),
    }
  })
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && (panelOpen = false)} />

<div class="shell">
  <aside class="side">
    <div class="brand">
      <img src="/favicon.png" alt="" width="22" height="22" />
      <span>Stickler</span>
    </div>
    <TemplateList {templates} {thumbs} selectedId={selected?.id ?? null} onselect={(t) => (selected = t)} />
    <button class="printer-chip tone-{chip.tone}" onclick={() => (panelOpen = true)}>
      <span class="dot" aria-hidden="true"></span>
      <span class="chip-text">
        <span class="chip-title">{chip.title}</span>
        <span class="chip-sub num">{chip.sub}</span>
      </span>
    </button>
  </aside>

  <main class="main">

    {#if selected}
      <Editor
        template={selected}
        {report}
        {presets}
        {icons}
        defaultDensity={density}
        {notify}
        onsaved={(t) => { delete thumbs[t.id]; loadTemplates(t.id) }}
        ondeleted={() => loadTemplates(templates[0]?.id)}
      />
    {/if}
  </main>
</div>

{#if panelOpen}
  <PrinterPanel {report} {checking} onrefresh={refresh} onclose={() => (panelOpen = false)} {notify} />
{/if}

{#if toast}
  <div class="toast {toast.kind}" role={toast.kind === 'error' ? 'alert' : 'status'}>{toast.msg}</div>
{/if}

<style>
  .shell {
    display: grid;
    grid-template-columns: 300px 1fr;
    height: 100%;
  }

  .side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--rule);
    background: var(--surface);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 18px 22px 14px;
    font-weight: 700;
    font-size: 16px;
    letter-spacing: -0.01em;
  }

  .brand img {
    border-radius: 5px;
  }

  .main {
    position: relative;
    min-width: 0;
    min-height: 0;
  }

  .printer-chip {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: auto 12px 12px;
    padding: 10px 12px;
    border: 1px solid var(--rule);
    border-radius: var(--radius-m);
    background: var(--surface-2);
    text-align: left;
  }

  .printer-chip:hover {
    border-color: var(--slate);
  }

  .chip-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .chip-title {
    font-weight: 600;
    font-size: 13.5px;
  }

  .chip-sub {
    color: var(--slate);
    font-size: 12.5px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--slate);
  }

  .tone-on .dot { background: var(--ok); }
  .tone-warn .dot { background: var(--accent); }
  .tone-off .dot { background: var(--danger); }
  .tone-idle .dot { animation: pulse 1.2s ease-in-out infinite; }

  @keyframes pulse {
    50% { opacity: 0.3; }
  }

  .toast {
    position: fixed;
    left: 50%;
    bottom: 92px;
    transform: translateX(-50%);
    max-width: min(560px, calc(100% - 32px));
    padding: 10px 16px;
    border-radius: var(--radius-m);
    background: var(--ink);
    color: var(--surface);
    box-shadow: var(--shadow-pop);
    animation: rise 180ms ease-out;
  }

  .toast.error {
    background: var(--danger);
    color: #fff;
  }

  @keyframes rise {
    from { transform: translate(-50%, 8px); opacity: 0; }
  }

  @media (max-width: 900px) {
    .shell {
      grid-template-columns: 240px 1fr;
    }
  }
</style>
