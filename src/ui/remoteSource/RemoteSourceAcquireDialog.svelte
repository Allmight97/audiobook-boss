<script lang="ts">
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

	// -- per-instance reactive state --

	let acquisition = $state(createInitialAcquisitionState());

	// -- controller / workflow instances bound to this component's state --

	const account = createAccountController(acquisition);
	const workflow = createAcquisitionWorkflow(acquisition);

	// -- selection / derived helpers (component-local because they close over view state) --

	function clearSelectedTitles(): void {
		acquisition.selectedTitleIds = new Set();
	}

	function toggleTitle(title: typeof acquisition.titles[number]): void {
		if (!isTitleAcquirable(title)) return;
		const next = new Set(acquisition.selectedTitleIds);
		if (next.has(title.titleId)) {
			next.delete(title.titleId);
		} else {
			next.add(title.titleId);
		}
		acquisition.selectedTitleIds = next;
	}

	function togglePdf(title: typeof acquisition.titles[number]): void {
		acquisition.includePdfByTitleId = {
			...acquisition.includePdfByTitleId,
			[title.titleId]: !acquisition.includePdfByTitleId[title.titleId],
		};
	}

	function visibleTitles(): typeof acquisition.titles {
		const { titles, titleFilter, showSupplementalPdfOnly, hideUnavailableTitles } = acquisition;
		const normalizedFilter = titleFilter.trim().toLowerCase();
		let facetTitles = showSupplementalPdfOnly
			? titles.filter((title) => title.supplementalPdfAvailable)
			: titles;
		if (hideUnavailableTitles) {
			facetTitles = facetTitles.filter(isTitleAcquirable);
		}
		if (!normalizedFilter) return facetTitles;
		return facetTitles.filter((title) =>
			[title.title, title.authors.join(' '), title.narrators.join(' ')]
				.join(' ')
				.toLowerCase()
				.includes(normalizedFilter),
		);
	}

	function selectedSummaryText(): string {
		const { selectedTitleIds } = acquisition;
		const count = selectedTitleIds.size;
		if (count === 0) return '0 selected';
		const visibleIds = new Set(visibleTitles().map((title) => title.titleId));
		const hiddenCount = [...selectedTitleIds].filter((id) => !visibleIds.has(id)).length;
		const titleLabel = count === 1 ? 'title' : 'titles';
		if (hiddenCount === 0) return `${count} ${titleLabel} selected`;
		const hiddenLabel = hiddenCount === 1 ? 'title' : 'titles';
		return `${count} ${titleLabel} selected (${hiddenCount} ${hiddenLabel} hidden by filter)`;
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
					<button class="btn-pill btn-pill-secondary" disabled={acquisition.isBusy} onclick={() => void account.logout()}>
						Logout
					</button>
				{/if}
				<button
					id="remote-source-close"
					class="btn-pill btn-pill-secondary"
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
						<button class="btn-pill btn-pill-primary" disabled={acquisition.isBusy} onclick={() => void account.startAuth()}>
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
						<button class="btn-pill btn-pill-secondary" disabled={acquisition.isBusy} onclick={() => void account.completeAuth()}>
							Complete Auth
						</button>
					</div>
				{:else}
					<div class="app-modal-field app-modal-field-button">
						<button class="btn-pill btn-pill-secondary" disabled={acquisition.isBusy} onclick={() => void account.loadLibrary()}>
							Refresh Library
						</button>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<button
							class="btn-pill btn-pill-primary"
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
				<div class="remote-source-status text-xs" aria-live="polite">{acquisition.statusMessage}</div>
			{/if}

			{#if acquisition.accountState?.status === 'connected'}
				<div class="remote-selection-summary" aria-live="polite">
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
						class="remote-progress-bar"
						role="progressbar"
						aria-label="Acquisition progress"
						aria-valuemin="0"
						aria-valuemax="100"
						aria-valuenow={Math.round(progressPercent(acquisition.activeJob))}
					>
						<span style={`width: ${progressPercent(acquisition.activeJob)}%;`}></span>
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
							class="btn-pill btn-pill-secondary remote-progress-cancel"
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
							<button
								type="button"
								class="remote-title-button"
								disabled={!isTitleAcquirable(title)}
								onclick={() => toggleTitle(title)}
							>
								<span class="remote-title-name">{title.title}</span>
								<span class="remote-title-meta">{title.authors.join(', ')}</span>
								{#if !isTitleAcquirable(title)}
									<span class="remote-title-availability">{titleAvailability(title).label}</span>
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
		gap: 0.5rem;
	}

	.remote-source-filter,
	.remote-source-handoff {
		grid-column: span 2;
	}

	.remote-source-status {
		min-height: 1rem;
		color: var(--text-secondary);
	}

	.remote-selection-summary {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		min-height: 1.5rem;
		color: var(--text-secondary);
		font-size: 0.75rem;
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
		gap: 0.25rem;
	}

	.remote-progress-bar {
		height: 0.45rem;
		overflow: hidden;
		border-radius: 999px;
		background-color: var(--bg-primary);
	}

	.remote-progress-bar span {
		display: block;
		height: 100%;
		border-radius: inherit;
		background-color: var(--accent-primary);
		transition: width 120ms ease-out;
	}

	.remote-progress-copy {
		display: flex;
		justify-content: space-between;
		gap: 0.75rem;
		color: var(--text-primary);
		font-size: 0.75rem;
	}

	.remote-progress-bytes {
		margin: 0;
		color: var(--text-muted);
		font-size: 0.7rem;
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
		gap: 0.5rem;
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

	.remote-title-button {
		display: flex;
		min-width: 0;
		flex: 1;
		flex-direction: column;
		gap: 0.125rem;
		padding: 0.65rem 0.75rem;
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
		font-size: 0.85rem;
		font-weight: 600;
	}

	.remote-title-meta,
	.remote-pdf-toggle,
	.remote-title-availability,
	.remote-title-availability-detail {
		color: var(--text-muted);
		font-size: 0.75rem;
	}

	.remote-title-availability {
		font-weight: 600;
		color: var(--text-secondary);
	}

	.remote-pdf-toggle {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		padding-right: 0.75rem;
		white-space: nowrap;
	}

	@media (max-width: 720px) {
		.remote-source-filter,
		.remote-source-handoff {
			grid-column: auto;
		}
	}
</style>
