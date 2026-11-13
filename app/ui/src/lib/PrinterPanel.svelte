<script lang="ts">
  import { api } from './api'
  import type { FoundPrinter, PrinterReport } from './api'

  let {
    report,
    checking,
    onrefresh,
    onclose,
    notify,
  }: {
    report: PrinterReport | null
    checking: boolean
    onrefresh: () => void
    onclose: () => void
    notify: (msg: string, kind?: 'ok' | 'error') => void
  } = $props()

  let scanning = $state(false)
  let found = $state<FoundPrinter[] | null>(null)
  let pairing = $state<string | null>(null)

  const colours: Record<string, string> = {
    white: '#ffffff',
    transparent: 'transparent',
    pink: '#f9c6d3',
    blue: '#bfdbfe',
    yellow: '#fde68a',
  }

  async function scan() {
    scanning = true
    found = null
    try {
      found = await api.discoverPrinters()
    } catch (e) {
      notify((e as Error).message, 'error')
    } finally {
      scanning = false
    }
  }

  async function pair(p: FoundPrinter) {
    pairing = p.mac
    try {
      await api.pairPrinter(p.mac, p.name)
      notify(`Paired with ${p.name}.`)
      found = null
      onrefresh()
    } catch (e) {
      notify((e as Error).message, 'error')
    } finally {
      pairing = null
    }
  }

  async function selfTest() {
    try {
      await api.selfTest()
      notify('Printing a self-test page.')
    } catch (e) {
      notify((e as Error).message, 'error')
    }
  }
</script>

<div class="scrim" role="presentation" onclick={onclose}></div>
<aside class="panel" aria-label="Printer">
  <header>
    <h2>Printer</h2>
    <button class="btn quiet" onclick={onclose} aria-label="Close">✕</button>
  </header>

  <section>
    {#if checking}
      <p class="muted">Checking the printer…</p>
    {:else if report?.connected}
      <dl>
        <dt>Printer</dt>
        <dd>{report.name ?? 'P21'}</dd>
        <dt>Status</dt>
        <dd>{report.status_text ?? 'Unknown'}</dd>
        {#if report.battery}
          <dt>Battery</dt>
          <dd class="num">{report.battery.percent}%{report.battery.charging ? ', charging' : ''}</dd>
        {/if}
        <dt>Labels</dt>
        <dd>
          {#if report.roll}
            <span class="roll">
              <span class="chip" style:background={colours[report.roll.color] ?? '#ddd'}></span>
              <span class="num">{report.roll.size.width_mm} × {report.roll.size.length_mm} mm</span>,
              {report.roll.color}, {report.roll.paper_type === 'continuous' ? 'continuous' : 'with gaps'}
            </span>
          {:else}
            Couldn't read the roll. Pick a label size when printing.
          {/if}
        </dd>
        {#if report.firmware}
          <dt>Firmware</dt>
          <dd class="num">{report.firmware}</dd>
        {/if}
      </dl>
    {:else}
      <p class="problem">{report?.error ?? 'Not connected.'}</p>
      <p class="muted">Turn the printer on and keep it nearby. If your phone app is connected to it, close the app first.</p>
    {/if}
    <button class="btn" onclick={onrefresh} disabled={checking}>Check again</button>
  </section>

  <section>
    <h3>Find a printer</h3>
    <p class="muted">Turn the P21 on, then search. Pairing needs no PIN.</p>
    <button class="btn" onclick={scan} disabled={scanning}>{scanning ? 'Searching… (12 s)' : 'Search'}</button>
    {#if found}
      {#if found.length === 0}
        <p class="muted">No P21 found. Is it switched on and close by?</p>
      {:else}
        <ul class="found">
          {#each found as p (p.mac)}
            <li>
              <span>{p.name} <span class="muted num">{p.mac}</span></span>
              <button class="btn primary" onclick={() => pair(p)} disabled={pairing !== null}>
                {pairing === p.mac ? 'Pairing…' : p.paired ? 'Use this printer' : 'Pair'}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {/if}
  </section>

  <details>
    <summary>Advanced</summary>
    <p class="muted">The self-test prints a page with the printer's settings.</p>
    <button class="btn" onclick={selfTest}>Print self-test</button>
  </details>
</aside>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(10, 12, 15, 0.25);
    animation: fade 160ms ease-out;
  }

  .panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(380px, 100%);
    background: var(--surface);
    box-shadow: var(--shadow-pop);
    padding: 18px 22px;
    overflow-y: auto;
    animation: slide 200ms cubic-bezier(0.2, 0.7, 0.2, 1);
  }

  @keyframes slide {
    from { transform: translateX(24px); opacity: 0; }
  }

  @keyframes fade {
    from { opacity: 0; }
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  h2 {
    margin: 0;
    font-size: 18px;
  }

  h3 {
    margin: 0 0 4px;
    font-size: 14px;
  }

  section {
    padding: 16px 0;
    border-bottom: 1px solid var(--rule);
  }

  dl {
    display: grid;
    grid-template-columns: 84px 1fr;
    gap: 8px 12px;
    margin: 0 0 14px;
  }

  dt {
    color: var(--slate);
  }

  dd {
    margin: 0;
  }

  .roll {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .chip {
    width: 14px;
    height: 14px;
    border-radius: 3px;
    box-shadow: 0 0 0 1px var(--rule);
  }

  .muted {
    color: var(--slate);
    margin: 4px 0 10px;
  }

  .problem {
    color: var(--danger);
    margin: 0 0 4px;
    font-weight: 550;
  }

  .found {
    list-style: none;
    padding: 0;
    margin: 12px 0 0;
  }

  .found li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 0;
  }

  details {
    padding: 14px 0;
  }

  summary {
    cursor: pointer;
    font-weight: 550;
  }
</style>
