<script lang="ts">
	import MetadataSurfacePanes from './MetadataSurfacePanes.svelte';
	import { readActiveFileSummary } from './activeFileSummary.svelte';

	const summary = $derived(readActiveFileSummary());
</script>

<aside class="metadata-rail" data-testid="metadata-rail" aria-label="Metadata editor">
	{#if !summary.hasSelection}
		<div class="metadata-rail-empty">Select a book to edit its details.</div>
	{:else}
		<header class="metadata-rail-head">
			{#if summary.coverDataUrl}
				<img class="metadata-rail-cover" src={summary.coverDataUrl} alt="" />
			{:else}
				<div class="metadata-rail-cover metadata-rail-cover-placeholder" aria-hidden="true"></div>
			{/if}
			<div class="metadata-rail-title-group">
				<h2>{summary.heading}</h2>
				{#if summary.railSubtitle}<p>{summary.railSubtitle}</p>{/if}
			</div>
		</header>
		<div class="metadata-rail-pane">
			<MetadataSurfacePanes idPrefix="metadata-rail" layout="stacked" />
		</div>
	{/if}
</aside>

<style>
	.metadata-rail {
		display: flex;
		min-width: 0;
		min-height: 0;
		flex: 1 1 auto;
		flex-direction: column;
		overflow: auto;
	}

	.metadata-rail-empty {
		display: grid;
		flex: 1 1 auto;
		place-items: center;
		padding: var(--space-5);
		color: var(--text-muted);
		font-size: var(--text-sm);
		text-align: center;
	}

	.metadata-rail-head {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		padding: var(--space-3) var(--space-4) 0;
	}

	.metadata-rail-cover {
		width: 56px;
		height: 56px;
		flex: 0 0 auto;
		border-radius: var(--radius-md);
		object-fit: cover;
	}

	.metadata-rail-cover-placeholder { background: var(--bg-hover); }
	.metadata-rail-title-group { min-width: 0; }
	.metadata-rail-title-group h2,
	.metadata-rail-title-group p { margin: 0; }
	.metadata-rail-title-group h2 { overflow: hidden; color: var(--text-primary); font-size: 15px; font-weight: 600; text-overflow: ellipsis; white-space: nowrap; }
	.metadata-rail-title-group p { overflow: hidden; margin-top: 2px; color: var(--text-muted); font-size: 12px; text-overflow: ellipsis; white-space: nowrap; }
	.metadata-rail-pane { padding: 0 var(--space-4) var(--space-4); }
</style>
