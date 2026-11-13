<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import type { FieldDef } from './api'
  import { iconSvg } from './icons'

  let { field, value = $bindable(), icons }: { field: FieldDef; value: string; icons: string[] } = $props()

  const id = $derived(`field-${field.key}`)
  let iconsOpen = $state(false)
  let iconQuery = $state('')
  const shownIcons = $derived(icons.filter((n) => !iconQuery || n.includes(iconQuery.toLowerCase())))
  const isToday = $derived(field.kind === 'date' && (value === '' || value === 'today'))
  // Local date (toISOString is UTC and gives tomorrow's date in the evening west of UTC).
  const now = new Date()
  const todayIso = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, '0')}-${String(now.getDate()).padStart(2, '0')}`

  async function pickImage() {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif', 'bmp'] }],
    })
    if (typeof path === 'string') value = path
  }
</script>

<div class="field" class:wide={field.kind === 'multiline' || field.kind === 'icon' || field.kind === 'qr_data'}>
  <label class="field-label" for={id}>{field.label}</label>

  {#if field.kind === 'multiline'}
    <textarea {id} class="input" rows="2" bind:value placeholder="One line per row (up to 3)"></textarea>
  {:else if field.kind === 'date'}
    <div class="row">
      <input
        {id}
        class="input"
        type="date"
        value={isToday ? todayIso : value}
        disabled={isToday}
        oninput={(e) => (value = (e.currentTarget as HTMLInputElement).value)}
      />
      <label class="check">
        <input type="checkbox" checked={isToday} onchange={(e) => (value = e.currentTarget.checked ? 'today' : todayIso)} />
        Today
      </label>
    </div>
  {:else if field.kind === 'icon'}
    <button {id} class="btn icon-btn" onclick={() => (iconsOpen = !iconsOpen)} aria-expanded={iconsOpen}>
      {#if value && iconSvg[value]}
        <span class="glyph" aria-hidden="true">{@html iconSvg[value]}</span>
      {/if}
      <span>{value || 'No icon'}</span>
      <span class="chev" aria-hidden="true">{iconsOpen ? '▴' : '▾'}</span>
    </button>
    {#if iconsOpen}
      <div class="icon-panel">
        <input class="input" type="search" placeholder="Search icons" bind:value={iconQuery} />
        <div class="icon-grid" role="listbox" aria-label="Icons">
          <button class="icon-cell none" role="option" aria-selected={!value} onclick={() => { value = ''; iconsOpen = false }}>
            None
          </button>
          {#each shownIcons as name (name)}
            <button
              class="icon-cell"
              class:on={name === value}
              role="option"
              aria-selected={name === value}
              title={name}
              onclick={() => { value = name; iconsOpen = false }}
            >
              <span class="glyph" aria-hidden="true">{@html iconSvg[name] ?? ''}</span>
              <span class="sr-only">{name}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  {:else if field.kind === 'image_path'}
    <div class="row">
      <input {id} class="input" readonly value={value ? value.split('/').pop() : ''} placeholder="No image chosen" />
      <button class="btn" onclick={pickImage}>Choose image…</button>
    </div>
  {:else}
    <input
      {id}
      class="input"
      bind:value
      spellcheck={field.kind === 'text'}
      placeholder={field.kind === 'qr_data' ? 'Text or link' : field.kind === 'barcode_data' ? 'Letters and numbers' : ''}
    />
  {/if}
</div>

<style>
  .field.wide {
    grid-column: 1 / -1;
  }

  textarea {
    resize: vertical;
    min-height: 58px;
  }

  .row {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .check {
    display: flex;
    gap: 6px;
    align-items: center;
    white-space: nowrap;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    text-align: left;
  }

  .chev {
    margin-left: auto;
    color: var(--slate);
  }

  .glyph {
    display: inline-flex;
    width: 22px;
    height: 22px;
  }

  .glyph :global(svg) {
    width: 100%;
    height: 100%;
  }

  .icon-panel {
    margin-top: 8px;
    padding: 10px;
    border: 1px solid var(--rule);
    border-radius: var(--radius-m);
    background: var(--surface);
  }

  .icon-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(40px, 1fr));
    gap: 4px;
    max-height: 200px;
    overflow-y: auto;
    margin-top: 8px;
  }

  .icon-cell {
    display: grid;
    place-items: center;
    height: 40px;
    border: 1px solid transparent;
    border-radius: var(--radius-s);
    background: transparent;
  }

  .icon-cell:hover {
    background: var(--surface-2);
  }

  .icon-cell.on {
    border-color: var(--ink);
    background: var(--accent-soft);
  }

  .icon-cell.none {
    font-size: 11px;
    color: var(--slate);
  }
</style>
