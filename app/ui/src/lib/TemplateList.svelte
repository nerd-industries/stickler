<script lang="ts">
  import type { Template } from './api'
  import { pngUrl } from './api'

  let {
    templates,
    thumbs,
    selectedId,
    onselect,
  }: {
    templates: Template[]
    thumbs: Record<string, string>
    selectedId: string | null
    onselect: (t: Template) => void
  } = $props()

  let query = $state('')

  const groups = $derived.by(() => {
    const q = query.trim().toLowerCase()
    const shown = templates.filter(
      (t) => !q || t.name.toLowerCase().includes(q) || t.category.toLowerCase().includes(q),
    )
    const mine = shown.filter((t) => !t.builtin)
    const byCat = new Map<string, Template[]>()
    for (const t of shown.filter((t) => t.builtin)) {
      byCat.set(t.category, [...(byCat.get(t.category) ?? []), t])
    }
    const out: [string, Template[]][] = []
    if (mine.length) out.push(['My templates', mine])
    return out.concat([...byCat.entries()])
  })
</script>

<div class="rail">
  <label class="search">
    <span class="sr-only">Search templates</span>
    <input class="input" type="search" placeholder="Search templates" bind:value={query} />
  </label>

  <nav class="list" aria-label="Templates">
    {#each groups as [category, items] (category)}
      <section>
        <h2>{category}</h2>
        {#each items as t (t.id)}
          <button
            class="item"
            class:selected={t.id === selectedId}
            aria-current={t.id === selectedId ? 'true' : undefined}
            onclick={() => onselect(t)}
          >
            <span class="thumb">
              {#if thumbs[t.id]}
                <img src={pngUrl(thumbs[t.id])} alt="" />
              {:else}
                <span class="thumb-empty" aria-hidden="true"></span>
              {/if}
            </span>
            <span class="name">{t.name}</span>
          </button>
        {/each}
      </section>
    {:else}
      <p class="empty">No templates match “{query}”.</p>
    {/each}
  </nav>
</div>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }

  .search {
    padding: 0 14px 10px;
  }

  .list {
    overflow-y: auto;
    padding: 0 8px 16px;
  }

  h2 {
    font-size: 12.5px;
    font-weight: 600;
    color: var(--slate);
    margin: 14px 8px 6px;
  }

  .item {
    display: grid;
    grid-template-columns: 92px 1fr;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 8px;
    border: 0;
    background: transparent;
    border-radius: var(--radius-s);
    text-align: left;
  }

  .item:hover {
    background: var(--surface-2);
  }

  .item.selected {
    background: var(--accent-soft);
  }

  .item.selected .name {
    font-weight: 600;
  }

  .thumb {
    display: block;
    line-height: 0;
  }

  .thumb img {
    width: 92px;
    height: auto;
    border-radius: 4px;
    box-shadow: 0 0 0 1px var(--rule);
  }

  .thumb-empty {
    display: block;
    width: 92px;
    aspect-ratio: 40 / 14;
    border-radius: 4px;
    background: var(--surface-2);
    box-shadow: 0 0 0 1px var(--rule);
  }

  .name {
    font-size: 13.5px;
  }

  .empty {
    color: var(--slate);
    padding: 12px 8px;
  }
</style>
