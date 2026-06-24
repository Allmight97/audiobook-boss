<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { tauriClient } from '../../lib/tauri/client';
	import {
		applyMetadataLookupResult,
		closeMetadataLookup,
		initMetadataLookup,
		searchMetadataLookup,
		skipMetadataLookupQueueItem,
		useManualMetadataEntryFromLookup,
	} from '../metadataLookup';
	import {
		cancelMetadataLookupCoverPreviewSchedule,
		getMetadataLookupCoverPreviewState,
		scheduleMetadataLookupCoverPreviews,
	} from './metadataLookupCoverPreview.svelte';
	import { metadataLookupState } from './state.svelte';

	function handleBackdropClick(event: MouseEvent): void {
		if (event.target === event.currentTarget) {
			closeMetadataLookup();
		}
	}

	function handleQueryKeyDown(event: KeyboardEvent): void {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		void searchMetadataLookup();
	}

	function formatPublicationDate(value: string | null): string {
		return value ? value.toString() : '—';
	}

	function formatDurationHours(durationSeconds: number | null): string {
		if (!durationSeconds) return '—';
		return `${Math.round(durationSeconds / 3600)}h`;
	}

	function formatSeriesSummary(result: (typeof metadataLookupState.results)[number]): string {
		const parts: string[] = [];
		if (result.series) {
			parts.push(
				result.seriesPart
					? `Series: ${result.series} #${result.seriesPart}`
					: `Series: ${result.series}`,
			);
		}
		if (result.subseries) {
			parts.push(
				result.subseriesPart
					? `Sub-series: ${result.subseries} #${result.subseriesPart}`
					: `Sub-series: ${result.subseries}`,
			);
		}
		return parts.length ? parts.join(' • ') : 'Series: —';
	}

	onMount(() => {
		initMetadataLookup();
	});

	$effect(() => {
		if (!metadataLookupState.isOpen || !metadataLookupState.hasSearched) {
			untrack(cancelMetadataLookupCoverPreviewSchedule);
			return;
		}

		const coverUrls = metadataLookupState.results.map((result) => result.coverUrl);
		untrack(() => {
			scheduleMetadataLookupCoverPreviews(coverUrls, tauriClient.loadCoverArtFromUrl);
		});

		return () => {
			untrack(cancelMetadataLookupCoverPreviewSchedule);
		};
	});
</script>

<div
	id="metadata-lookup-modal"
	class="app-modal-backdrop"
	class:open={metadataLookupState.isOpen}
	data-testid="metadata-lookup-modal"
	aria-hidden={!metadataLookupState.isOpen}
	onclick={handleBackdropClick}
>
	<div
		class="app-modal-dialog"
		role="dialog"
		aria-modal="true"
		aria-labelledby="metadata-lookup-title"
	>
		<div class="app-modal-header">
			<h3 id="metadata-lookup-title">Find Metadata Online</h3>
			<button
				id="metadata-lookup-close"
				class="btn-pill btn-pill-secondary"
				data-testid="metadata-lookup-close"
				type="button"
				onclick={closeMetadataLookup}
			>
				Close
			</button>
		</div>
		<div class="app-modal-body">
			<div class="app-modal-controls">
				<div class="app-modal-field">
					<label for="metadata-lookup-query">Search</label>
					<input
						id="metadata-lookup-query"
						type="text"
						placeholder="Title, author, or ASIN (e.g., B01234ABCD)"
						data-testid="metadata-lookup-query"
						bind:value={metadataLookupState.query}
						onkeydown={handleQueryKeyDown}
					/>
				</div>
				<div class="app-modal-field">
					<label for="metadata-lookup-source">Source</label>
					<select
						id="metadata-lookup-source"
						data-testid="metadata-lookup-source"
						bind:value={metadataLookupState.source}
					>
						<option value="auto">Auto (All Sources)</option>
						<option value="audnexus">Audnexus</option>
						<option value="openlibrary">OpenLibrary</option>
					</select>
				</div>
				<div class="app-modal-field">
					<label for="metadata-lookup-apply-mode">Apply</label>
					<select
						id="metadata-lookup-apply-mode"
						data-testid="metadata-lookup-apply-mode"
						bind:value={metadataLookupState.applyMode}
					>
						<option value="current"
							>{metadataLookupState.isQueueMode ? 'Apply to current file' : 'Apply to file'}</option
						>
						{#if metadataLookupState.isQueueMode}
							<option value="queue">Apply & next in queue</option>
						{/if}
					</select>
				</div>
				<div class="app-modal-field app-modal-field-toggle">
					<label class="checkbox-label text-xs mb-0">
						<input
							type="checkbox"
							id="metadata-lookup-cover-toggle"
							data-testid="metadata-lookup-cover-toggle"
							bind:checked={metadataLookupState.replaceCoverArt}
						/>
						<span class="option-label">Replace cover art</span>
					</label>
				</div>
				<div class="app-modal-field app-modal-field-button">
					<button
						id="metadata-lookup-search-btn"
						class="btn-pill btn-pill-primary"
						data-testid="metadata-lookup-search-btn"
						type="button"
						onclick={() => void searchMetadataLookup()}
					>
						Search
					</button>
				</div>
				<div class="app-modal-field app-modal-field-button">
					<button
						id="metadata-lookup-skip-btn"
						class="btn-pill btn-pill-secondary"
						data-testid="metadata-lookup-skip-btn"
						type="button"
						disabled={!metadataLookupState.skipEnabled}
						onclick={() => void skipMetadataLookupQueueItem()}
					>
						Skip
					</button>
				</div>
			</div>
			<div
				id="metadata-lookup-context"
				class="metadata-lookup-context text-xs muted-text"
			>
				{metadataLookupState.queueContext}
			</div>
			<div
				id="metadata-lookup-status"
				class="metadata-lookup-status text-xs"
				class:is-error={metadataLookupState.statusVariant === 'error'}
				class:is-success={metadataLookupState.statusVariant === 'success'}
			>
				{metadataLookupState.statusMessage}
			</div>
			<div
				id="metadata-lookup-results"
				class="app-modal-results"
				>
					{#if metadataLookupState.hasSearched && metadataLookupState.results.length === 0}
						<div class="metadata-lookup-empty muted-text">
							<p>No online matches found across Audnexus and OpenLibrary.</p>
							<p class="text-xs" style="margin-top: 0.5rem;">
								Older CD-era or rare audiobook editions may not be indexed. Use manual entry to finish metadata for this file.
							</p>
							<button
								id="metadata-lookup-manual-entry-btn"
								class="btn-pill btn-pill-secondary mt-2"
								data-testid="metadata-lookup-manual-entry-btn"
								type="button"
								onclick={useManualMetadataEntryFromLookup}
							>
								Use Manual Entry
							</button>
						</div>
					{/if}
				{#each metadataLookupState.results as result, index}
					<div class="app-modal-result">
						<div
							class="metadata-lookup-cover"
							role="presentation"
						>
							{#if result.coverUrl}
								{@const preview = getMetadataLookupCoverPreviewState(result.coverUrl)}
								{#if preview.status === 'ready'}
									<img
										src={preview.dataUrl}
										alt={`${result.title} cover art`}
										data-testid="metadata-lookup-cover-image"
									/>
								{:else if preview.status === 'loading' || preview.status === 'queued'}
									<span data-testid="metadata-lookup-cover-loading">Loading…</span>
								{:else if preview.status === 'error'}
									<span data-testid="metadata-lookup-cover-error">Preview failed</span>
								{:else}
									<span data-testid="metadata-lookup-cover-available">Art Available</span>
								{/if}
							{:else}
								<span>No Art</span>
							{/if}
						</div>
						<div class="metadata-lookup-details">
							<div class="metadata-lookup-title">{result.title}</div>
							<div class="metadata-lookup-meta">
								{result.authors.length
									? `Author: ${result.authors.join(', ')}`
									: 'Author: —'}
							</div>
							<div class="metadata-lookup-meta">
								{result.narrators.length
									? `Narrator: ${result.narrators.join(', ')}`
									: 'Narrator: —'}
							</div>
							<div class="metadata-lookup-meta">{formatSeriesSummary(result)}</div>
							<div class="metadata-lookup-meta">
								Publication {formatPublicationDate(result.publishedDate ?? null)} • Length: {formatDurationHours(
									result.durationSeconds ?? null,
								)}
							</div>
						<span
							class="metadata-lookup-source"
							class:is-secondary-source={result.source === 'openlibrary'}
						>
							{result.source === 'audnexus'
								? (result.audibleOnly ? 'Audible-only' : 'Audnexus')
								: result.source === 'openlibrary'
									? 'OpenLibrary'
									: result.source}
						</span>
						</div>
						<div class="metadata-lookup-actions">
							<button
								type="button"
								class="btn-pill btn-pill-secondary"
								data-index={index}
								onclick={() => void applyMetadataLookupResult(index)}
							>
								Use Metadata
							</button>
						</div>
					</div>
				{/each}
			</div>
		</div>
	</div>
</div>

<style>
	.metadata-lookup-cover {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 4rem;
		height: 4rem;
		overflow: hidden;
		border: 1px solid var(--border-secondary);
		border-radius: 0.375rem;
		background: var(--bg-drag-area);
		color: var(--text-muted);
		font-size: 0.7rem;
		text-align: center;
	}

	.metadata-lookup-cover img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.metadata-lookup-details {
		min-width: 0;
	}

	.metadata-lookup-title {
		margin-bottom: 0.25rem;
		font-weight: 600;
	}

	.metadata-lookup-meta {
		font-size: 0.75rem;
		color: var(--text-secondary);
	}

	.metadata-lookup-source {
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
		margin-top: 0.25rem;
		padding: 0.15rem 0.5rem;
		border: 1px solid var(--border-secondary);
		border-radius: 9999px;
		color: var(--text-secondary);
		font-size: 0.7rem;
	}

	.metadata-lookup-source.is-secondary-source {
		color: var(--text-muted);
	}

	.metadata-lookup-actions {
		display: flex;
		align-items: flex-start;
	}

	.metadata-lookup-empty {
		padding: 0.75rem;
		border: 1px dashed var(--border-secondary);
		border-radius: 0.375rem;
		color: var(--text-muted);
		text-align: center;
	}

	.metadata-lookup-context,
	.metadata-lookup-status {
		min-height: 1rem;
	}

	.metadata-lookup-status.is-error {
		color: var(--text-error, #ef4444);
	}

	.metadata-lookup-status.is-success {
		color: var(--text-success, #10b981);
	}

	@media (max-width: 720px) {
		.app-modal-controls {
			grid-template-columns: minmax(0, 1fr);
		}

		.app-modal-field-button,
		.app-modal-field-toggle {
			align-items: stretch;
		}
	}
</style>
