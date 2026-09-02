import { createEffect, createSignal, For, onSettled } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../../app/runtime';
import { Button, CoverThumb, Dialog } from '../foundation';
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
	const coverPreview = useAppRuntime().lookup.coverPreview;
	const previewState = () => coverPreview(props.coverUrl);
	const readyUrl = () => {
		const state = previewState();
		return state.status === 'ready' ? state.dataUrl : '';
	};

	return (
		<CoverThumb>
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
		</CoverThumb>
	);
}

export function MetadataLookupView(): JSX.Element {
	const lookup = useAppRuntime().lookup;
	const view = lookup.view;
	const runLookup = lookup.run;
	const setTitleQuery = lookup.setTitleQuery;
	const setAuthorQuery = lookup.setAuthorQuery;
	const setSource = lookup.setSource;
	const setApplyMode = lookup.setApplyMode;
	const setReplaceCover = lookup.setReplaceCover;
	const [restoreFocus, setRestoreFocus] = createSignal(true);

	onSettled(() => {
		void runLookup({ type: 'init' });
	});

	createEffect(
		() => {
			const state = view();
			return {
				isOpen: state.isOpen,
				hasSearched: state.hasSearched,
				coverUrls: state.results.map((result) => result.coverUrl),
			};
		},
		(state) => {
			if (!state.isOpen || !state.hasSearched) {
				lookup.cancelCoverPreviews();
				return;
			}
			lookup.scheduleCoverPreviews(state.coverUrls);
			return () => lookup.cancelCoverPreviews();
		},
	);

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
			<Dialog.Header>
				<h3 id="metadata-lookup-title">Find Metadata Online</h3>
				<Button id="metadata-lookup-close" data-testid="metadata-lookup-close" onClick={close}>
					Close
				</Button>
			</Dialog.Header>
			<Dialog.Body>
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
						<label class="checkbox-label tight">
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
						<Button
							id="metadata-lookup-search-btn"
							tone="primary"
							data-testid="metadata-lookup-search-btn"
							onClick={() => void runLookup({ type: 'search' })}
						>
							Search
						</Button>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<Button
							id="metadata-lookup-skip-btn"
							data-testid="metadata-lookup-skip-btn"
							disabled={!view().skipEnabled}
							onClick={() => void runLookup({ type: 'skipQueueItem' })}
						>
							Skip
						</Button>
					</div>
				</div>
				<div id="metadata-lookup-context" class="metadata-lookup-context muted-text">
					{view().queueContext}
				</div>
				<Dialog.Status
					id="metadata-lookup-status"
					tone={
						view().statusVariant === 'error'
							? 'error'
							: view().statusVariant === 'success'
								? 'success'
								: 'neutral'
					}
					class="metadata-lookup-status"
				>
					{view().statusMessage}
				</Dialog.Status>
				<div id="metadata-lookup-results" class="app-modal-results">
					{view().hasSearched && view().results.length === 0 && (
						<div class="app-modal-empty muted-text">
							<p>No online matches found across Audnexus and OpenLibrary.</p>
							<p class="metadata-lookup-empty-hint">
								Older CD-era or rare audiobook editions may not be indexed. Use manual entry to
								finish metadata for this file.
							</p>
							<Button
								id="metadata-lookup-manual-entry-btn"
								class="metadata-lookup-manual-entry"
								data-testid="metadata-lookup-manual-entry-btn"
								onClick={() => {
									setRestoreFocus(false);
									void runLookup({ type: 'manualEntry' });
								}}
							>
								Use Manual Entry
							</Button>
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
										class={[
											'metadata-lookup-source',
											{ 'is-secondary-source': result.source === 'openlibrary' },
										]}
									>
										{sourceLabel(result)}
									</span>
								</div>
								<div class="metadata-lookup-actions">
									<Button
										data-index={index()}
										onClick={() => void runLookup({ type: 'applyResult', index: index() })}
									>
										Use Metadata
									</Button>
								</div>
							</div>
						)}
					</For>
				</div>
			</Dialog.Body>
		</Dialog>
	);
}
