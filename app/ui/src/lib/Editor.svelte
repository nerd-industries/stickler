<script lang="ts">
  import { untrack } from 'svelte'
  import { api, pngUrl, sizeLabel } from './api'
  import type { Align, Border, FontChoice, PrinterReport, Style, Template, Values } from './api'
  import FieldInput from './FieldInput.svelte'

  let {
    template,
    report,
    presets,
    icons,
    defaultDensity,
    onsaved,
    ondeleted,
    notify,
  }: {
    template: Template
    report: PrinterReport | null
    presets: string[]
    icons: string[]
    defaultDensity: number
    onsaved: (t: Template) => void
    ondeleted: (id: string) => void
    notify: (msg: string, kind?: 'ok' | 'error') => void
  } = $props()

  let values = $state<Values>({})
  let style = $state<Style>(untrack(() => ({ ...template.style })))
  let size = $state('auto')
  let copies = $state(1)
  let density = $state(15)
  let png = $state<string | null>(null)
  let previewError = $state<string | null>(null)
  let previewSize = $state('14x40')
  let printing = $state(false)
  let ejecting = $state(false)
  let saving = $state(false)
  let saveName = $state('')

  // Reset the form whenever a different template is chosen.
  let loadedId = ''
  $effect(() => {
    if (template.id !== loadedId) {
      loadedId = template.id
      values = Object.fromEntries(template.fields.map((f) => [f.key, f.default]))
      style = { ...template.style }
      saving = false
    }
  })
  $effect(() => {
    density = defaultDensity
  })

  const rollSize = $derived(report?.roll ? `${report.roll.size.width_mm}x${report.roll.size.length_mm}` : null)

  // Debounced preview; only the newest request may update the image.
  let seq = 0
  $effect(() => {
    const args = [template.id, $state.snapshot(values), $state.snapshot(style), size === 'auto' ? rollSize : size] as const
    const mine = ++seq
    const timer = setTimeout(async () => {
      try {
        const p = await api.preview(args[0], args[1] as Values, args[2] as Style, args[3])
        if (mine !== seq) return
        png = p.png_base64
        previewSize = p.size
        previewError = null
      } catch (e) {
        if (mine === seq) previewError = (e as Error).message
      }
    }, 150)
    return () => clearTimeout(timer)
  })

  async function print() {
    if (printing) return
    printing = true
    try {
      const r = await api.print({
        template_id: template.id,
        values: $state.snapshot(values),
        style: $state.snapshot(style),
        size: size === 'auto' ? null : size,
        copies,
        density,
      })
      ejecting = true
      setTimeout(() => (ejecting = false), 700)
      const n = copies === 1 ? '1 label' : `${copies} labels`
      notify(`Printed ${n} on ${sizeLabel(r.size)} (size ${r.source_text}).`)
    } catch (e) {
      notify((e as Error).message, 'error')
    } finally {
      printing = false
    }
  }

  async function saveAs() {
    const name = saveName.trim()
    if (!name) return
    try {
      const t = await api.saveTemplate({
        ...$state.snapshot(template),
        id: '',
        name,
        category: 'Mine',
        builtin: false,
        style: $state.snapshot(style),
        fields: template.fields.map((f) => ({ ...f, default: values[f.key] ?? f.default })),
      })
      saving = false
      saveName = ''
      notify(`Saved “${t.name}” to My templates.`)
      onsaved(t)
    } catch (e) {
      notify((e as Error).message, 'error')
    }
  }

  async function remove() {
    try {
      await api.deleteTemplate(template.id)
      notify(`Deleted “${template.name}”.`)
      ondeleted(template.id)
    } catch (e) {
      notify((e as Error).message, 'error')
    }
  }

  function onKey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && (e.key === 'Enter' || e.key === 'p')) {
      e.preventDefault()
      print()
    }
  }

  const fonts: [FontChoice, string][] = [
    ['sans', 'Sans'],
    ['condensed', 'Narrow'],
    ['rounded', 'Rounded'],
  ]
  const aligns: [Align, string][] = [
    ['left', 'Left'],
    ['center', 'Centre'],
    ['right', 'Right'],
  ]
  const borders: Border[] = ['none', 'thin', 'thick', 'rounded', 'double']
  const hasText = $derived(template.fields.some((f) => f.kind === 'text' || f.kind === 'multiline' || f.kind === 'date'))
  // The preview is drawn at 4× (32 px per mm); show it at real proportions.
  const [pw, pl] = $derived(previewSize.split('x').map(Number))
</script>

<svelte:window onkeydown={onKey} />

<div class="editor">
  <header class="head">
    <h1>{template.name}</h1>
    <div class="head-actions">
      {#if !template.builtin}
        <button class="btn quiet" onclick={remove}>Delete</button>
      {/if}
      <button class="btn quiet" onclick={() => (saving = !saving)} aria-expanded={saving}>Save as template</button>
    </div>
  </header>

  {#if saving}
    <form class="save-row" onsubmit={(e) => { e.preventDefault(); saveAs() }}>
      <label class="sr-only" for="save-name">Template name</label>
      <input id="save-name" class="input" placeholder="Name, e.g. Pantry jar" bind:value={saveName} />
      <button class="btn primary" type="submit" disabled={!saveName.trim()}>Save</button>
    </form>
  {/if}

  <div class="slot" aria-live="polite">
    <div class="tape">
      {#if png}
        <img
          class="label"
          class:ejecting
          src={pngUrl(png)}
          alt="Preview of the {template.name} label"
          style:aspect-ratio="{pl} / {pw}"
        />
      {/if}
    </div>
    <p class="slot-caption num">
      {#if previewError}
        <span class="err">{previewError}</span>
      {:else}
        {sizeLabel(previewSize)}
        {#if size === 'auto' && report?.roll}· from the loaded roll{/if}
      {/if}
    </p>
  </div>

  <div class="form">
    <div class="fields">
      {#each template.fields as f (template.id + f.key)}
        <FieldInput field={f} bind:value={values[f.key]} {icons} />
      {/each}
    </div>

    {#if hasText}
      <div class="style-row" role="group" aria-label="Text style">
        <div class="seg" role="radiogroup" aria-label="Font">
          {#each fonts as [v, label] (v)}
            <button role="radio" aria-checked={style.font === v} class:on={style.font === v} onclick={() => (style.font = v)}>
              {label}
            </button>
          {/each}
        </div>
        <button class="toggle" class:on={style.bold} aria-pressed={style.bold} onclick={() => (style.bold = !style.bold)}>
          <b>B</b><span class="sr-only">Bold</span>
        </button>
        <div class="seg" role="radiogroup" aria-label="Alignment">
          {#each aligns as [v, label] (v)}
            <button role="radio" aria-checked={style.align === v} class:on={style.align === v} onclick={() => (style.align = v)}>
              {label}
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="style-row" role="group" aria-label="Frame">
      <div class="swatches" role="radiogroup" aria-label="Border">
        {#each borders as b (b)}
          <button
            role="radio"
            aria-checked={style.border === b}
            class="swatch b-{b}"
            class:on={style.border === b}
            title="{b} border"
            onclick={() => (style.border = b)}
          >
            <span class="sr-only">{b} border</span>
          </button>
        {/each}
      </div>
      <button class="toggle wide" class:on={style.invert} aria-pressed={style.invert} onclick={() => (style.invert = !style.invert)}>
        White on black
      </button>
    </div>
  </div>

  <footer class="bar">
    <label class="ctl">
      <span class="field-label">Label size</span>
      <select class="input" bind:value={size}>
        <option value="auto">{rollSize ? `From printer (${sizeLabel(rollSize)})` : 'From printer'}</option>
        {#each presets as p (p)}
          <option value={p}>{sizeLabel(p)}</option>
        {/each}
      </select>
    </label>
    <label class="ctl small">
      <span class="field-label">Copies</span>
      <input class="input num" type="number" min="1" max="50" bind:value={copies} />
    </label>
    <label class="ctl small">
      <span class="field-label">Darkness</span>
      <input class="input num" type="number" min="1" max="15" bind:value={density} />
    </label>
    <button class="btn primary print" onclick={print} disabled={printing || !!previewError}>
      {printing ? 'Printing…' : copies > 1 ? `Print ${copies}` : 'Print'}
    </button>
  </footer>
</div>

<style>
  .editor {
    display: grid;
    grid-template-rows: auto auto auto 1fr auto;
    height: 100%;
    min-height: 0;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 28px 6px;
  }

  h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }

  .head-actions {
    display: flex;
    gap: 4px;
  }

  .save-row {
    display: flex;
    gap: 8px;
    padding: 6px 28px 0;
    max-width: 520px;
  }

  /* The printer slot: a dark tape strip the label sits on. */
  .slot {
    padding: 14px 28px 4px;
  }

  .tape {
    display: grid;
    place-items: center;
    padding: 28px 24px;
    background: linear-gradient(var(--tape), var(--tape)) padding-box;
    border-radius: var(--radius-m);
    box-shadow: inset 0 2px 0 var(--tape-edge), inset 0 -2px 0 var(--tape-edge);
    overflow: hidden;
    min-height: 180px;
  }

  .label {
    width: min(100%, 620px);
    height: auto;
    display: block;
    image-rendering: pixelated;
    filter: drop-shadow(0 6px 14px rgba(0, 0, 0, 0.35));
    transition: transform 0ms;
  }

  .label.ejecting {
    animation: eject 680ms cubic-bezier(0.5, 0, 0.2, 1);
  }

  @keyframes eject {
    0% { transform: translateX(0); opacity: 1; }
    55% { transform: translateX(-115%); opacity: 1; }
    56% { transform: translateX(115%); opacity: 0; }
    100% { transform: translateX(0); opacity: 1; }
  }

  .slot-caption {
    margin: 8px 2px 0;
    color: var(--slate);
    font-size: 12.5px;
  }

  .err {
    color: var(--danger);
  }

  .form {
    overflow-y: auto;
    padding: 10px 28px 20px;
  }

  .fields {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 14px 18px;
    max-width: 760px;
  }

  .style-row {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    align-items: center;
    margin-top: 18px;
  }

  .seg {
    display: inline-flex;
    border: 1px solid var(--rule);
    border-radius: var(--radius-s);
    overflow: hidden;
    background: var(--surface);
  }

  .seg button {
    border: 0;
    background: transparent;
    padding: 6px 12px;
  }

  .seg button + button {
    border-left: 1px solid var(--rule);
  }

  .seg button.on,
  .toggle.on {
    background: var(--ink);
    color: var(--surface);
  }

  .toggle {
    border: 1px solid var(--rule);
    background: var(--surface);
    border-radius: var(--radius-s);
    min-width: 34px;
    padding: 6px 10px;
  }

  .swatches {
    display: inline-flex;
    gap: 6px;
  }

  .swatch {
    width: 44px;
    height: 26px;
    border-radius: 4px;
    background: var(--surface);
    border: 1px solid var(--rule);
    position: relative;
  }

  .swatch.on {
    outline: 2px solid var(--ink);
    outline-offset: 1px;
  }

  .swatch::after {
    content: '';
    position: absolute;
    inset: 4px;
  }

  .b-thin::after { border: 1.5px solid var(--ink); }
  .b-thick::after { border: 3px solid var(--ink); }
  .b-rounded::after { border: 2px solid var(--ink); border-radius: 5px; }
  .b-double::after { border: 3px double var(--ink); }

  .bar {
    display: flex;
    align-items: flex-end;
    gap: 14px;
    padding: 14px 28px 18px;
    border-top: 1px solid var(--rule);
    background: var(--surface);
  }

  .ctl {
    display: block;
    min-width: 200px;
  }

  .ctl.small {
    min-width: 0;
    width: 92px;
  }

  .print {
    margin-left: auto;
    min-width: 140px;
    padding: 10px 22px;
    font-size: 15px;
  }
</style>
