<script lang="ts">
	import {
		applyMetadataLookupResult,
		closeMetadataLookup,
		searchMetadataLookup,
		skipMetadataLookupQueueItem,
		useManualMetadataEntryFromLookup,
	} from '../metadataLookup';
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

	function formatYear(value: number | null): string {
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
</script>

<div
	id="metadata-lookup-modal"
	class="metadata-lookup-modal"
	class:open={metadataLookupState.isOpen}
	data-testid="metadata-lookup-modal"
	aria-hidden={!metadataLookupState.isOpen}
	on:click={handleBackdropClick}
>
	<div
		class="metadata-lookup-dialog"
		role="dialog"
		aria-modal="true"
		aria-labelledby="metadata-lookup-title"
	>
		<div class="metadata-lookup-header">
			<h3 id="metadata-lookup-title">Find Metadata Online</h3>
			<button
				id="metadata-lookup-close"
				class="btn-pill btn-pill-secondary"
				data-testid="metadata-lookup-close"
				type="button"
				on:click={closeMetadataLookup}
			>
				Close
			</button>
		</div>
		<div class="metadata-lookup-body">
			<div class="metadata-lookup-controls">
				<div class="metadata-lookup-field">
					<label for="metadata-lookup-query">Search</label>
					<input
						id="metadata-lookup-query"
						type="text"
						placeholder="Title, author, or ASIN (e.g., B01234ABCD)"
						data-testid="metadata-lookup-query"
						bind:value={metadataLookupState.query}
						on:keydown={handleQueryKeyDown}
					/>
				</div>
				<div class="metadata-lookup-field">
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
				<div class="metadata-lookup-field">
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
				<div class="metadata-lookup-field metadata-lookup-field-toggle">
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
				<div class="metadata-lookup-field metadata-lookup-field-button">
					<button
						id="metadata-lookup-search-btn"
						class="btn-pill btn-pill-primary"
						data-testid="metadata-lookup-search-btn"
						type="button"
						on:click={() => void searchMetadataLookup()}
					>
						Search
					</button>
				</div>
				<div class="metadata-lookup-field metadata-lookup-field-button">
					<button
						id="metadata-lookup-skip-btn"
						class="btn-pill btn-pill-secondary"
						data-testid="metadata-lookup-skip-btn"
						type="button"
						disabled={!metadataLookupState.skipEnabled}
						on:click={() => void skipMetadataLookupQueueItem()}
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
				class="metadata-lookup-results"
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
								on:click={useManualMetadataEntryFromLookup}
							>
								Use Manual Entry
							</button>
						</div>
					{/if}
				{#each metadataLookupState.results as result, index}
					<div class="metadata-lookup-result">
						<div class="metadata-lookup-cover">
							{#if result.coverUrl}
								<img src={result.coverUrl} alt={`${result.title} cover art`} loading="lazy" />
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
								Year: {formatYear(result.publishedYear)} • Length: {formatDurationHours(
									result.durationSeconds,
								)}
							</div>
						<span
							class="metadata-lookup-source"
							class:is-fallback={result.source === 'openlibrary'}
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
								on:click={() => void applyMetadataLookupResult(index)}
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
