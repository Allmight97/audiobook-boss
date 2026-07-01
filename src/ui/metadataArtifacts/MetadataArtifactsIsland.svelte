<script lang="ts">
	import {
		metadataArtifactsState,
		stageMetadataArtifactClear,
	} from './state.svelte';

	let expanded = $state(false);

	const hasAnyValue = $derived(
		metadataArtifactsState.rows.some((row) => row.value !== null || row.clearPending),
	);
</script>

<section class="metadata-artifacts" data-testid="metadata-artifacts" aria-label="Metadata Artifacts">
	<button
		type="button"
		class="metadata-artifacts-toggle"
		data-testid="metadata-artifacts-toggle"
		aria-expanded={expanded}
		onclick={() => {
			expanded = !expanded;
		}}
	>
		<span class="metadata-artifacts-caret" class:open={expanded}>▸</span>
		Embedded artifacts
		{#if hasAnyValue && !expanded}
			<span class="metadata-artifacts-hint">present</span>
		{/if}
	</button>

	{#if expanded}
		<p class="text-xs muted-text">
			Sort, provenance, and compatibility tags other apps may display. Saves keep
			them unless you clear them here; a clear applies on the next metadata save.
		</p>
		{#if metadataArtifactsState.multiSelection}
			<p class="text-xs muted-text" data-testid="metadata-artifacts-multi">
				Select a single file to inspect its embedded artifacts.
			</p>
		{:else if metadataArtifactsState.filePath === null}
			<p class="text-xs muted-text" data-testid="metadata-artifacts-empty">
				No file selected.
			</p>
		{:else}
			<div class="metadata-artifacts-rows">
				{#each metadataArtifactsState.rows as row (row.field)}
					<div class="metadata-artifacts-row" data-testid="metadata-artifact-{row.field}">
						<span class="metadata-artifacts-label">{row.label}</span>
						<span class="metadata-artifacts-value" title={row.value ?? undefined}>
							{#if row.clearPending}
								<em data-testid="metadata-artifact-{row.field}-pending">cleared (pending save)</em>
							{:else}
								{row.value ?? '—'}
							{/if}
						</span>
						<button
							type="button"
							class="btn-pill btn-pill-secondary metadata-artifacts-clear"
							data-testid="metadata-artifact-{row.field}-clear"
							disabled={row.value === null || row.clearPending}
							onclick={() => stageMetadataArtifactClear(row.field)}
						>
							Clear
						</button>
					</div>
				{/each}
			</div>
		{/if}
	{/if}
</section>

<style>
	.metadata-artifacts {
		margin-top: 0.6rem;
		min-width: 0;
	}

	.metadata-artifacts-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		background: none;
		border: none;
		padding: 0;
		font-size: 0.78rem;
		font-weight: 600;
		color: var(--text-secondary);
		cursor: pointer;
	}

	.metadata-artifacts-caret {
		display: inline-block;
		transition: transform 0.12s ease;
	}

	.metadata-artifacts-caret.open {
		transform: rotate(90deg);
	}

	.metadata-artifacts-hint {
		font-weight: 400;
		font-size: 0.7rem;
		color: var(--text-secondary);
		border: 1px solid var(--border-color, currentColor);
		border-radius: 999px;
		padding: 0 0.4rem;
	}

	.metadata-artifacts-rows {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		margin-top: 0.4rem;
	}

	.metadata-artifacts-row {
		display: grid;
		grid-template-columns: minmax(8rem, auto) minmax(0, 1fr) auto;
		gap: 0.6rem;
		align-items: center;
		font-size: 0.78rem;
	}

	.metadata-artifacts-label {
		color: var(--text-secondary);
	}

	.metadata-artifacts-value {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--text-primary);
	}

	.metadata-artifacts-clear {
		font-size: 0.7rem;
		padding: 0.1rem 0.6rem;
	}
</style>
