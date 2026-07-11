<script lang="ts">
	import { untrack } from 'svelte';
	import { tauriClient } from '../../lib/tauri/client';
	import { closeRemoteSourceAcquire, remoteSourceAcquireState } from './state.svelte';

	// provider-neutral state shape (owned by acquisitionState.svelte.ts)
	import { createInitialAcquisitionState } from './acquisitionState.svelte';

	// account / library lifecycle controller (owned by acquisitionAccount.ts)
	import { createAccountController } from './acquisitionAccount';

	// acquisition workflow controller (owned by acquisitionWorkflow.ts)
	import { createAcquisitionWorkflow } from './acquisitionWorkflow';

	// pure rendering utilities (owned by remoteSourceAcquireDialogHelpers.ts)
	import {
		bytesLabel,
		isAcquisitionTerminal,
		isTitleAcquirable,
		titleAvailability,
		progressPercent,
		progressTitleLabel,
	} from './remoteSourceAcquireDialogHelpers';
	import {
		selectedRemoteTitleSummaryText,
		toggledRemoteTitleSelection,
		toggledSupplementalPdfPreference,
		visibleRemoteTitles,
	} from './remoteSourceSelection';
	import {
		cancelRemoteSourceCoverPreviewSchedule,
		getRemoteSourceCoverPreviewState,
		scheduleRemoteSourceCoverPreviews,
	} from './remoteSourceCoverPreview.svelte';

	// -- per-instance reactive state --

	let acquisition = $state(createInitialAcquisitionState());

	// -- controller / workflow instances bound to this component's state --

	const account = createAccountController(acquisition);
	const workflow = createAcquisitionWorkflow(acquisition);

	// -- selection / derived assignments delegate pure policy to helpers --

	function clearSelectedTitles(): void {
		acquisition.selectedTitleIds = new Set();
	}

	function toggleTitle(title: typeof acquisition.titles[number]): void {
		acquisition.selectedTitleIds = toggledRemoteTitleSelection(
			acquisition.selectedTitleIds,
			title,
		);
	}

	function togglePdf(title: typeof acquisition.titles[number]): void {
		acquisition.includePdfByTitleId = toggledSupplementalPdfPreference(
			acquisition.includePdfByTitleId,
			title.titleId,
		);
	}

	function visibleTitles(): typeof acquisition.titles {
		return visibleRemoteTitles(acquisition.titles, {
			titleFilter: acquisition.titleFilter,
			showSupplementalPdfOnly: acquisition.showSupplementalPdfOnly,
			hideUnavailableTitles: acquisition.hideUnavailableTitles,
		});
	}

	function selectedSummaryText(): string {
		return selectedRemoteTitleSummaryText(acquisition.selectedTitleIds, visibleTitles());
	}

	// -- modal lifecycle --

	function handleBackdropClick(event: MouseEvent): void {
		if (event.target === event.currentTarget) {
			closeRemoteSourceAcquire();
		}
	}

	$effect(() => {
		if (remoteSourceAcquireState.isOpen && !acquisition.didHydrateOpenDialog) {
			acquisition.didHydrateOpenDialog = true;
			void account.hydrateOpenDialog();
		}
		if (!remoteSourceAcquireState.isOpen) {
			acquisition.didHydrateOpenDialog = false;
		}
	});

	$effect(() => {
		if (!remoteSourceAcquireState.isOpen || acquisition.accountState?.status !== 'connected') {
			untrack(cancelRemoteSourceCoverPreviewSchedule);
			return;
		}

		const coverUrls = visibleTitles().map((title) => title.coverUrl);
		untrack(() => {
			scheduleRemoteSourceCoverPreviews(coverUrls, tauriClient.loadCoverArtFromUrl);
		});

		return () => {
			untrack(cancelRemoteSourceCoverPreviewSchedule);
		};
	});
</script>

<div
	id="remote-source-modal"
	class="app-modal-backdrop"
	class:open={remoteSourceAcquireState.isOpen}
	data-testid="remote-source-modal"
	aria-hidden={!remoteSourceAcquireState.isOpen}
	onclick={handleBackdropClick}
>
	<div
		class="app-modal-dialog remote-source-dialog"
		role="dialog"
		aria-modal="true"
		aria-labelledby="remote-source-title"
	>
		<div class="app-modal-header">
			<h3 id="remote-source-title">Acquire Audiobooks</h3>
			<div class="remote-source-header-actions">
				{#if acquisition.accountState?.status === 'connected'}
					<button
						class="btn-pill btn-pill-secondary btn-pill-sm"
						disabled={acquisition.isBusy}
						onclick={() => void account.logout()}
					>
						Logout
					</button>
				{/if}
				<button
					id="remote-source-close"
					class="btn-pill btn-pill-secondary btn-pill-sm"
					type="button"
					onclick={closeRemoteSourceAcquire}
				>
					Close
				</button>
			</div>
		</div>

		<div class="app-modal-body">
			<div class="app-modal-controls">
				<div class="app-modal-field">
					<label for="remote-source-provider">Source</label>
					<select id="remote-source-provider" disabled>
						<option>Audible</option>
					</select>
				</div>

				{#if acquisition.accountState?.status !== 'connected'}
					<div class="app-modal-field app-modal-field-button">
						<button
							class="btn-pill btn-pill-primary btn-pill-sm"
							disabled={acquisition.isBusy}
							onclick={() => void account.startAuth()}
						>
							Connect Audible
						</button>
					</div>
					<div class="app-modal-field remote-source-handoff">
						<label for="remote-source-handoff">Auth Handoff</label>
						<input
							id="remote-source-handoff"
							type="text"
							placeholder="Final Amazon URL or handoff file path"
							bind:value={acquisition.handoffPath}
						/>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<button
							class="btn-pill btn-pill-secondary btn-pill-sm"
							disabled={acquisition.isBusy}
							onclick={() => void account.completeAuth()}
						>
							Complete Auth
						</button>
					</div>
				{:else}
					<div class="app-modal-field app-modal-field-button">
						<button
							class="btn-pill btn-pill-secondary btn-pill-sm"
							disabled={acquisition.isBusy}
							onclick={() => void account.loadLibrary()}
						>
							Refresh Library
						</button>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<button
							class="btn-pill btn-pill-primary btn-pill-sm"
							disabled={acquisition.isBusy || acquisition.selectedTitleIds.size === 0}
							onclick={() => void workflow.acquireSelected()}
						>
							Acquire Selected
						</button>
					</div>
					<div class="app-modal-field remote-source-filter">
						<label for="remote-source-filter">Filter</label>
						<input
							id="remote-source-filter"
							type="search"
							placeholder="Filter loaded titles"
							bind:value={acquisition.titleFilter}
						/>
					</div>
					<div class="app-modal-field app-modal-field-toggle remote-source-pdf-filter">
						<label class="checkbox-label text-xs mb-0">
							<input type="checkbox" bind:checked={acquisition.showSupplementalPdfOnly} />
							<span class="option-label">Supplemental PDF only</span>
						</label>
					</div>
					<div class="app-modal-field app-modal-field-toggle remote-source-availability-filter">
						<label class="checkbox-label text-xs mb-0">
							<input type="checkbox" bind:checked={acquisition.hideUnavailableTitles} />
							<span class="option-label">Hide unavailable</span>
						</label>
					</div>
				{/if}
			</div>

			{#if acquisition.statusMessage}
				<div class="remote-source-status app-modal-status text-xs" aria-live="polite">{acquisition.statusMessage}</div>
			{/if}

			{#if acquisition.accountState?.status === 'connected'}
				<div class="remote-selection-summary app-modal-status" aria-live="polite">
					<span>{selectedSummaryText()}</span>
					{#if acquisition.selectedTitleIds.size > 0}
						<button
							class="remote-clear-selection"
							type="button"
							disabled={acquisition.isBusy}
							onclick={clearSelectedTitles}
						>
							Clear selection
						</button>
					{/if}
				</div>
			{/if}

			{#if acquisition.activeJob?.progress}
				<div class="remote-progress" role="status" aria-live="polite">
					<div
						class="app-progress-track"
						role="progressbar"
						aria-label="Acquisition progress"
						aria-valuemin="0"
						aria-valuemax="100"
						aria-valuenow={Math.round(progressPercent(acquisition.activeJob))}
					>
						<span class="app-progress-fill" style={`width: ${progressPercent(acquisition.activeJob)}%;`}></span>
					</div>
					<div class="remote-progress-copy">
						<span>{progressTitleLabel(acquisition.activeJob.progress, acquisition.titles)}</span>
						<span>{Math.round(progressPercent(acquisition.activeJob))}%</span>
					</div>
					{#if bytesLabel(acquisition.activeJob.progress)}
						<p class="remote-progress-bytes">{bytesLabel(acquisition.activeJob.progress)}</p>
					{/if}
					{#if !isAcquisitionTerminal(acquisition.activeJob)}
						<button
							class="btn-pill btn-pill-secondary btn-pill-sm remote-progress-cancel"
							type="button"
							onclick={() => void workflow.cancelActiveAcquisition()}
						>
							Cancel Acquisition
						</button>
					{/if}
				</div>
			{/if}

			{#if acquisition.accountState?.status === 'connected'}
				<div
					class="app-modal-results remote-title-list"
					role="listbox"
					aria-label="Audible titles"
					aria-multiselectable="true"
				>
					{#each visibleTitles() as title (title.titleId)}
						<div
							class="remote-title-row"
							class:selected={acquisition.selectedTitleIds.has(title.titleId)}
							class:unavailable={!isTitleAcquirable(title)}
							role="option"
							aria-selected={acquisition.selectedTitleIds.has(title.titleId)}
							aria-disabled={!isTitleAcquirable(title)}
						>
							<div
								class="app-cover-thumb remote-title-cover"
								role="presentation"
								data-testid="remote-title-cover"
							>
								{#if title.coverUrl}
									{@const preview = getRemoteSourceCoverPreviewState(title.coverUrl)}
									{#if preview.status === 'ready'}
										<img
											src={preview.dataUrl}
											alt={`${title.title} cover art`}
											data-testid="remote-title-cover-image"
										/>
									{:else if preview.status === 'loading' || preview.status === 'queued'}
										<span data-testid="remote-title-cover-loading">Loading…</span>
									{:else if preview.status === 'error'}
										<span data-testid="remote-title-cover-error">Preview failed</span>
									{:else}
										<span data-testid="remote-title-cover-available">Art Available</span>
									{/if}
								{:else}
									<span data-testid="remote-title-cover-missing">No Art</span>
								{/if}
							</div>
							<button
								type="button"
								class="remote-title-button"
								disabled={!isTitleAcquirable(title)}
								onclick={() => toggleTitle(title)}
							>
								<span class="remote-title-name">{title.title}</span>
								<span class="remote-title-meta">{title.authors.join(', ')}</span>
								{#if !isTitleAcquirable(title)}
									<span class="remote-title-availability app-badge app-badge-warn"
										>{titleAvailability(title).label}</span
									>
									{#if titleAvailability(title).detail}
										<span class="remote-title-availability-detail">{titleAvailability(title).detail}</span>
									{/if}
								{/if}
							</button>
							{#if title.supplementalPdfAvailable}
								<label class="remote-pdf-toggle">
									<input
										type="checkbox"
										disabled={!isTitleAcquirable(title)}
										checked={acquisition.includePdfByTitleId[title.titleId] ?? true}
										onchange={() => togglePdf(title)}
									/>
									PDF
								</label>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>

<style>
	.remote-source-dialog {
		width: min(62rem, calc(100vw - 2rem));
	}

	.remote-source-header-actions {
		display: flex;
		gap: var(--space-2);
	}

	.remote-source-filter,
	.remote-source-handoff {
		grid-column: span 2;
	}

	.remote-selection-summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--space-3);
		min-height: 1.5rem;
		color: var(--text-secondary);
		font-size: var(--text-sm);
	}

	.remote-clear-selection {
		padding: 0;
		border: 0;
		background: transparent;
		color: var(--accent-primary);
		font: inherit;
		font-weight: 600;
		cursor: pointer;
	}

	.remote-clear-selection:disabled {
		color: var(--text-muted);
		cursor: default;
	}

	.remote-progress {
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
	}

	.remote-progress-copy {
		display: flex;
		justify-content: space-between;
		gap: var(--space-3);
		color: var(--text-primary);
		font-size: var(--text-sm);
	}

	.remote-progress-bytes {
		margin: 0;
		color: var(--text-muted);
		font-size: var(--text-xs);
	}

	.remote-progress-cancel {
		align-self: flex-start;
	}

	.remote-title-list {
		max-height: min(36rem, 55vh);
		overflow-y: auto;
		padding: 0;
	}

	.remote-title-row {
		display: flex;
		align-items: center;
		gap: var(--density-pad);
		border-bottom: 1px solid var(--border-primary);
	}

	.remote-title-row:last-child {
		border-bottom: none;
	}

	.remote-title-row.selected {
		background-color: var(--bg-hover);
	}

	.remote-title-row.unavailable {
		opacity: 0.7;
	}

	.remote-title-cover {
		flex: 0 0 var(--cover-thumb-size);
		margin-left: var(--density-pad);
	}

	.remote-title-button {
		display: flex;
		min-width: 0;
		flex: 1;
		flex-direction: column;
		gap: calc(var(--space-1) / 2);
		padding: var(--density-pad) var(--density-pad) var(--density-pad) 0;
		border: 0;
		background: transparent;
		color: inherit;
		text-align: left;
		cursor: pointer;
	}

	.remote-title-button:disabled {
		cursor: default;
	}

	.remote-title-name,
	.remote-title-meta,
	.remote-title-availability,
	.remote-title-availability-detail {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.remote-title-name {
		font-size: var(--text-md);
		font-weight: 600;
	}

	.remote-title-meta,
	.remote-pdf-toggle,
	.remote-title-availability,
	.remote-title-availability-detail {
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.remote-title-availability {
		font-weight: 600;
		align-self: flex-start;
	}

	.remote-pdf-toggle {
		display: flex;
		align-items: center;
		gap: var(--space-1);
		padding-right: var(--density-pad);
		white-space: nowrap;
	}

	@media (max-width: 720px) {
		.remote-source-filter,
		.remote-source-handoff {
			grid-column: auto;
		}
	}
</style>
