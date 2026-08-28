import { createEffect, createSignal, For, onCleanup, onMount, type JSX } from 'solid-js';
import {
	bumpLookupPreviewAtom,
	cancelMetadataLookupCoverPreviewSchedule,
	getMetadataLookupCoverPreviewState,
	lookupPreviewRevisionAtom,
	lookupViewAtom,
	runLookupActionAtom,
	scheduleMetadataLookupCoverPreviews,
	setLookupApplyModeAtom,
	setLookupAuthorQueryAtom,
	setLookupReplaceCoverArtAtom,
	setLookupSourceAtom,
	setLookupTitleQueryAtom,
	subscribeMetadataLookupCoverPreviews,
} from '../../app/metadataLookup';
import { metadataCapabilityAtom } from '../../app/metadataSession';
import { useAtomSet, useAtomValue } from '../../app/runtime/solid';
import { Dialog } from '../../lib/ui/Dialog';
import type { OnlineMetadataResult } from '../../types/metadata';
import './metadataLookup.css';

function formatPublicationDate(value: string | null | undefined): string {
	return value ? value.toString() : '—';
}

function formatDurationHours(durationSeconds: number | null | undefined): string {
	if (!durationSeconds) return '—';
	return `${Math.round(durationSeconds / 3600)}h`;
}

function formatSeriesSummary(result: OnlineMetadataResult): string {
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

function sourceLabel(result: OnlineMetadataResult): string {
	if (result.source === 'audnexus') {
		return result.audibleOnly ? 'Audible-only' : 'Audnexus';
	}
	if (result.source === 'openlibrary') {
		return 'OpenLibrary';
	}
	return result.source;
}

function LookupCoverThumb(props: {
	readonly coverUrl: string | null | undefined;
	readonly title: string;
}): JSX.Element {
	const previewRevision = useAtomValue(() => lookupPreviewRevisionAtom);
	const previewState = () => {
		previewRevision();
		return getMetadataLookupCoverPreviewState(props.coverUrl);
	};
	const readyUrl = () => {
		const state = previewState();
		return state.status === 'ready' ? state.dataUrl : '';
	};

	return (
		<div class="app-cover-thumb" role="presentation">
			{props.coverUrl ? (
				previewState().status === 'ready' ? (
					<img
						src={readyUrl()}
						alt={`${props.title} cover art`}
						data-testid="metadata-lookup-cover-image"
					/>
				) : previewState().status === 'loading' || previewState().status === 'queued' ? (
					<span data-testid="metadata-lookup-cover-loading">Loading…</span>
				) : previewState().status === 'error' ? (
					<span data-testid="metadata-lookup-cover-error">Preview failed</span>
				) : (
					<span data-testid="metadata-lookup-cover-available">Art Available</span>
				)
			) : (
				<span>No Art</span>
			)}
		</div>
	);
}

export function MetadataLookupView(): JSX.Element {
	const view = useAtomValue(() => lookupViewAtom);
	const runLookup = useAtomSet(() => runLookupActionAtom);
	const setTitleQuery = useAtomSet(() => setLookupTitleQueryAtom);
	const setAuthorQuery = useAtomSet(() => setLookupAuthorQueryAtom);
	const setSource = useAtomSet(() => setLookupSourceAtom);
	const setApplyMode = useAtomSet(() => setLookupApplyModeAtom);
	const setReplaceCover = useAtomSet(() => setLookupReplaceCoverArtAtom);
	const bumpPreview = useAtomSet(() => bumpLookupPreviewAtom);
	const capability = useAtomValue(() => metadataCapabilityAtom);
	const [restoreFocus, setRestoreFocus] = createSignal(true);

	onMount(() => {
		void runLookup({ type: 'init' });
		onCleanup(subscribeMetadataLookupCoverPreviews(() => bumpPreview(undefined)));
	});

	createEffect(() => {
		const state = view();
		if (!state.isOpen || !state.hasSearched) {
			cancelMetadataLookupCoverPreviewSchedule();
			return;
		}
		scheduleMetadataLookupCoverPreviews(
			state.results.map((result) => result.coverUrl),
			(url) => capability().loadCoverArtFromUrl(url),
		);
	});

	function handleQueryKeyDown(event: KeyboardEvent): void {
		if (event.key !== 'Enter') return;
		event.preventDefault();
		void runLookup({ type: 'search' });
	}

	function close(): void {
		setRestoreFocus(true);
		void runLookup({ type: 'close' });
	}

	return (
		<Dialog
			id="metadata-lookup-modal"
			open={view().isOpen}
			onClose={close}
			labelledBy="metadata-lookup-title"
			testId="metadata-lookup-modal"
			restoreFocus={restoreFocus()}
		>
			<div class="app-modal-header">
				<h3 id="metadata-lookup-title">Find Metadata Online</h3>
				<button
					id="metadata-lookup-close"
					class="btn-pill btn-pill-secondary"
					data-testid="metadata-lookup-close"
					type="button"
					onClick={close}
				>
					Close
				</button>
			</div>
			<div class="app-modal-body">
				<div class="app-modal-controls">
					<div class="app-modal-field app-modal-field-stack">
						<div class="app-modal-field">
							<label for="metadata-lookup-title-query">Title</label>
							<input
								id="metadata-lookup-title-query"
								type="text"
								placeholder="Book title or ASIN (e.g., B01234ABCD)"
								data-testid="metadata-lookup-title-query"
								value={view().titleQuery}
								onInput={(event) => setTitleQuery(event.currentTarget.value)}
								onKeyDown={handleQueryKeyDown}
							/>
						</div>
						<div class="app-modal-field">
							<label for="metadata-lookup-author-query">Author</label>
							<input
								id="metadata-lookup-author-query"
								type="text"
								placeholder="Author"
								data-testid="metadata-lookup-author-query"
								value={view().authorQuery}
								onInput={(event) => setAuthorQuery(event.currentTarget.value)}
								onKeyDown={handleQueryKeyDown}
							/>
						</div>
					</div>
					<div class="app-modal-field">
						<label for="metadata-lookup-source">Source</label>
						<select
							id="metadata-lookup-source"
							data-testid="metadata-lookup-source"
							value={view().source}
							onChange={(event) => {
								const next = event.currentTarget.value;
								if (next === 'auto' || next === 'audnexus' || next === 'openlibrary') {
									setSource(next);
								}
							}}
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
							value={view().applyMode}
							onChange={(event) =>
								setApplyMode(event.currentTarget.value === 'queue' ? 'queue' : 'current')
							}
						>
							<option value="current">
								{view().isQueueMode ? 'Apply to current file' : 'Apply to file'}
							</option>
							{view().isQueueMode && <option value="queue">Apply & next in queue</option>}
						</select>
					</div>
					<div class="app-modal-field app-modal-field-toggle">
						<label class="checkbox-label text-xs mb-0">
							<input
								type="checkbox"
								id="metadata-lookup-cover-toggle"
								data-testid="metadata-lookup-cover-toggle"
								checked={view().replaceCoverArt}
								onChange={(event) => setReplaceCover(event.currentTarget.checked)}
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
							onClick={() => void runLookup({ type: 'search' })}
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
							disabled={!view().skipEnabled}
							onClick={() => void runLookup({ type: 'skipQueueItem' })}
						>
							Skip
						</button>
					</div>
				</div>
				<div id="metadata-lookup-context" class="metadata-lookup-context text-xs muted-text">
					{view().queueContext}
				</div>
				<div
					id="metadata-lookup-status"
					class="metadata-lookup-status app-modal-status text-xs"
					classList={{
						'is-error': view().statusVariant === 'error',
						'is-success': view().statusVariant === 'success',
					}}
				>
					{view().statusMessage}
				</div>
				<div id="metadata-lookup-results" class="app-modal-results">
					{view().hasSearched && view().results.length === 0 && (
						<div class="app-modal-empty muted-text">
							<p>No online matches found across Audnexus and OpenLibrary.</p>
							<p class="text-xs" style={{ 'margin-top': '0.5rem' }}>
								Older CD-era or rare audiobook editions may not be indexed. Use manual entry to
								finish metadata for this file.
							</p>
							<button
								id="metadata-lookup-manual-entry-btn"
								class="btn-pill btn-pill-secondary mt-2"
								data-testid="metadata-lookup-manual-entry-btn"
								type="button"
								onClick={() => {
									setRestoreFocus(false);
									void runLookup({ type: 'manualEntry' });
								}}
							>
								Use Manual Entry
							</button>
						</div>
					)}
					<For each={view().results}>
						{(result, index) => (
							<div class="app-modal-result">
								<LookupCoverThumb coverUrl={result.coverUrl} title={result.title} />
								<div class="metadata-lookup-details">
									<div class="metadata-lookup-title">{result.title}</div>
									<div class="metadata-lookup-meta">
										{result.authors.length ? `Author: ${result.authors.join(', ')}` : 'Author: —'}
									</div>
									<div class="metadata-lookup-meta">
										{result.narrators.length
											? `Narrator: ${result.narrators.join(', ')}`
											: 'Narrator: —'}
									</div>
									<div class="metadata-lookup-meta">{formatSeriesSummary(result)}</div>
									<div class="metadata-lookup-meta">
										Publication {formatPublicationDate(result.publishedDate ?? null)} • Length:{' '}
										{formatDurationHours(result.durationSeconds ?? null)}
									</div>
									<span
										class="metadata-lookup-source"
										classList={{ 'is-secondary-source': result.source === 'openlibrary' }}
									>
										{sourceLabel(result)}
									</span>
								</div>
								<div class="metadata-lookup-actions">
									<button
										type="button"
										class="btn-pill btn-pill-secondary"
										data-index={index()}
										onClick={() => void runLookup({ type: 'applyResult', index: index() })}
									>
										Use Metadata
									</button>
								</div>
							</div>
						)}
					</For>
				</div>
			</div>
		</Dialog>
	);
}
