<script lang="ts">
	import { tick } from 'svelte';
	import { normalizeAppError } from '../../lib/tauri/appError';
	import { tauriClient } from '../../lib/tauri/client';
	import type {
		AcquisitionJob,
		AcquisitionProgress,
		ProviderId,
		RemoteSourceAccountState,
		RemoteTitle,
		RemoteTitleAvailability,
	} from '../../types/remoteSource';
	import {
		getImportedAudioPathsBlockedMessage,
		handleImportedAudioPaths,
	} from '../fileImport/handlers';
	import { getCurrentFileList } from '../fileList/state.svelte';
	import { closeRemoteSourceAcquire, remoteSourceAcquireState } from './state.svelte';
	import { registerRemoteSourceSupplementalAssets } from './sessionAssets.svelte';

	const providerId: ProviderId = 'audible';
	const acquisitionPollDelayMs = 100;

	type AcquisitionJobWithProgress = AcquisitionJob & {
		progress?: AcquisitionProgress;
	};
	type RemoteSourceDiagnostic = AcquisitionJob['diagnostics'][number];

	let isBusy = $state(false);
	let didHydrateOpenDialog = $state(false);
	let accountState = $state<RemoteSourceAccountState | null>(null);
	let titles = $state<RemoteTitle[]>([]);
	let selectedTitleIds = $state<Set<string>>(new Set());
	let includePdfByTitleId = $state<Record<string, boolean>>({});
	let titleFilter = $state('');
	let showSupplementalPdfOnly = $state(false);
	let hideUnavailableTitles = $state(false);
	let handoffPath = $state('');
	let statusMessage = $state('');
	let activeJob = $state<AcquisitionJobWithProgress | null>(null);
	let lastJob = $state<AcquisitionJobWithProgress | null>(null);

	function setError(cause: unknown, fallback: string): void {
		const error = normalizeAppError(cause, fallback);
		console.error(`${fallback} code=${error.code} category=${error.category}`);
		statusMessage = error.code === 'unknown_error' ? fallback : error.message;
	}

	function delay(ms: number): Promise<void> {
		return new Promise((resolve) => window.setTimeout(resolve, ms));
	}

	function acquisitionJobIsTerminal(job: AcquisitionJobWithProgress): boolean {
		return (
			job.progress?.terminal === true ||
			job.status === 'failed' ||
			job.status === 'cancelled' ||
			job.status === 'validated' ||
			job.status === 'importedToFileList'
		);
	}

	function diagnosticStatus(diagnostics: RemoteSourceDiagnostic[]): string {
		const uniqueMessages: string[] = [];
		const seenMessages = new Set<string>();
		for (const diagnostic of diagnostics) {
			const message = diagnostic.message.trim();
			if (!message || seenMessages.has(message)) continue;
			seenMessages.add(message);
			uniqueMessages.push(message);
		}
		return uniqueMessages.join(' ');
	}

	function statusFromAcquisitionJob(job: AcquisitionJobWithProgress): string {
		const diagnostics = diagnosticStatus(job.diagnostics);
		if (job.progress?.terminal && diagnostics) return diagnostics;
		if (job.progress?.message) return job.progress.message;
		return diagnostics || 'Audible acquisition is running.';
	}

	function progressPercent(job: AcquisitionJobWithProgress): number {
		return Math.max(0, Math.min(100, job.progress?.percentage ?? 0));
	}

	function clearedHandoffJob(job: AcquisitionJobWithProgress): AcquisitionJobWithProgress {
		return {
			...job,
			materializedFiles: [],
			supplementalAssets: [],
		};
	}

	function bytesLabel(progress: AcquisitionProgress): string | null {
		if (progress.bytesDownloaded == null) return null;
		const downloadedMb = progress.bytesDownloaded / (1024 * 1024);
		if (progress.bytesTotal == null || progress.bytesTotal <= 0) {
			return `${downloadedMb.toFixed(1)} MB downloaded`;
		}
		const totalMb = progress.bytesTotal / (1024 * 1024);
		return `${downloadedMb.toFixed(1)} / ${totalMb.toFixed(1)} MB`;
	}

	function progressTitleLabel(progress: AcquisitionProgress): string {
		const title = titles.find((candidate) => candidate.titleId === progress.currentTitleId);
		const ordinal =
			progress.currentItemIndex != null && progress.totalItems != null
				? `${progress.currentItemIndex}/${progress.totalItems}`
				: null;
		const titleLabel = title?.title ?? (progress.currentTitleId ? 'Selected title' : null);
		const context = [ordinal, titleLabel].filter(Boolean).join(' - ');
		if (!context) return progress.message;
		return `${progress.message.replace(/\.$/, '')}: ${context}`;
	}

	function handleBackdropClick(event: MouseEvent): void {
		if (event.target === event.currentTarget) {
			closeRemoteSourceAcquire();
		}
	}

	async function pollAcquisitionToTerminal(
		initialJob: AcquisitionJobWithProgress,
	): Promise<AcquisitionJobWithProgress> {
		let currentJob = initialJob;
		while (!acquisitionJobIsTerminal(currentJob)) {
			await delay(acquisitionPollDelayMs);
			currentJob = (await tauriClient.getRemoteSourceAcquisitionStatus(
				currentJob.jobId,
			)) as AcquisitionJobWithProgress;
			activeJob = currentJob;
			lastJob = currentJob;
			statusMessage = statusFromAcquisitionJob(currentJob);
		}
		return currentJob;
	}

	async function finishAcquisitionJob(job: AcquisitionJobWithProgress): Promise<void> {
		const materializedPaths = job.materializedFiles.map((file) => file.path);
		if (materializedPaths.length > 0) {
			const importResult = await handleImportedAudioPaths(materializedPaths);
			if (importResult.status !== 'imported') {
				await tauriClient.purgeRemoteSourceSession(job.jobId);
				const cleanedJob = clearedHandoffJob(job);
				activeJob = cleanedJob;
				lastJob = cleanedJob;
				statusMessage = `${importResult.message} Staged remote files were removed; retry acquisition after processing completes.`;
				return;
			}
			registerRemoteSourceSupplementalAssets(job, getCurrentFileList());
			statusMessage = `${materializedPaths.length} acquired title${materializedPaths.length === 1 ? '' : 's'} imported.`;
		} else {
			statusMessage =
				diagnosticStatus(job.diagnostics) ||
				'Audible acquisition did not materialize an importable file.';
		}
	}

	async function refreshAccountState(): Promise<void> {
		accountState = await tauriClient.getRemoteSourceAccountState(providerId);
	}

	async function hydrateOpenDialog(): Promise<void> {
		isBusy = true;
		try {
			await refreshAccountState();
			if (accountState?.status === 'connected') {
				await loadLibrary();
			}
		} catch (cause) {
			setError(cause, 'Failed to load remote source state.');
		} finally {
			isBusy = false;
		}
	}

	async function startAuth(): Promise<void> {
		isBusy = true;
		try {
			const response = await tauriClient.startRemoteSourceAuth(providerId);
			statusMessage = response.message;
			await tauriClient.openUrl(response.authorizationUrl);
		} catch (cause) {
			setError(cause, 'Failed to start Audible auth.');
		} finally {
			isBusy = false;
		}
	}

	async function completeAuth(): Promise<void> {
		isBusy = true;
		try {
			accountState = await tauriClient.completeRemoteSourceAuth({
				providerId,
				responseUrlHandoffPath: handoffPath.trim() || undefined,
			});
			statusMessage = 'Audible connected.';
			await loadLibrary();
		} catch (cause) {
			setError(cause, 'Failed to complete Audible auth.');
		} finally {
			isBusy = false;
		}
	}

	async function logout(): Promise<void> {
		isBusy = true;
		try {
			accountState = await tauriClient.logoutRemoteSourceAccount(providerId);
			titles = [];
			selectedTitleIds = new Set();
			includePdfByTitleId = {};
			activeJob = null;
			lastJob = null;
			statusMessage = 'Audible disconnected.';
		} catch (cause) {
			setError(cause, 'Failed to disconnect Audible.');
		} finally {
			isBusy = false;
		}
	}

	async function loadLibrary(): Promise<void> {
		isBusy = true;
		try {
			const library = await tauriClient.loadRemoteSourceLibrary(providerId);
			titles = library.titles;
			const selectableTitleIds = new Set(
				library.titles
					.filter((title) => titleIsAcquirable(title))
					.map((title) => title.titleId),
			);
			selectedTitleIds = new Set(
				[...selectedTitleIds].filter((titleId) => selectableTitleIds.has(titleId)),
			);
			includePdfByTitleId = Object.fromEntries(
				library.titles.map((title) => [title.titleId, title.supplementalPdfAvailable]),
			);
			statusMessage =
				library.diagnostics.length > 0
					? diagnosticStatus(library.diagnostics)
					: `${library.titles.length} Audible titles loaded.`;
		} catch (cause) {
			setError(cause, 'Failed to load Audible library.');
		} finally {
			isBusy = false;
		}
	}

	function clearSelectedTitles(): void {
		selectedTitleIds = new Set();
	}

	function toggleTitle(title: RemoteTitle): void {
		if (!titleIsAcquirable(title)) return;
		const next = new Set(selectedTitleIds);
		if (next.has(title.titleId)) {
			next.delete(title.titleId);
		} else {
			next.add(title.titleId);
		}
		selectedTitleIds = next;
	}

	function togglePdf(title: RemoteTitle): void {
		includePdfByTitleId = {
			...includePdfByTitleId,
			[title.titleId]: !includePdfByTitleId[title.titleId],
		};
	}

	function titleAvailability(title: RemoteTitle): RemoteTitleAvailability {
		return (
			title.availability ?? {
				status: title.unsupportedReasons.length > 0 ? 'providerUnavailable' : 'available',
				acquirable: title.unsupportedReasons.length === 0,
				label: title.unsupportedReasons.length > 0 ? 'Unavailable from Audible' : 'Available',
				detail:
					title.unsupportedReasons.length > 0
						? 'Audible reports this title is not playable or downloadable for this account.'
						: undefined,
			}
		);
	}

	function titleIsAcquirable(title: RemoteTitle): boolean {
		return titleAvailability(title).acquirable;
	}

	function filteredTitles(): RemoteTitle[] {
		const normalizedFilter = titleFilter.trim().toLowerCase();
		let facetTitles = showSupplementalPdfOnly
			? titles.filter((title) => title.supplementalPdfAvailable)
			: titles;
		if (hideUnavailableTitles) {
			facetTitles = facetTitles.filter(titleIsAcquirable);
		}
		if (!normalizedFilter) return facetTitles;
		return facetTitles.filter((title) =>
			[title.title, title.authors.join(' '), title.narrators.join(' ')]
				.join(' ')
				.toLowerCase()
				.includes(normalizedFilter),
		);
	}

	function selectedOutsideFilterCount(): number {
		if (!titleFilter.trim() && !showSupplementalPdfOnly && !hideUnavailableTitles) return 0;
		const visibleTitleIds = new Set(filteredTitles().map((title) => title.titleId));
		return [...selectedTitleIds].filter((titleId) => !visibleTitleIds.has(titleId)).length;
	}

	function selectedTitleSummary(): string {
		const count = selectedTitleIds.size;
		if (count === 0) return '0 selected';
		const hiddenCount = selectedOutsideFilterCount();
		const titleLabel = count === 1 ? 'title' : 'titles';
		if (hiddenCount === 0) return `${count} ${titleLabel} selected`;
		const hiddenLabel = hiddenCount === 1 ? 'title' : 'titles';
		return `${count} ${titleLabel} selected (${hiddenCount} ${hiddenLabel} hidden by filter)`;
	}

	async function acquireSelected(): Promise<void> {
		if (selectedTitleIds.size === 0) {
			statusMessage = 'Select at least one Audible title.';
			return;
		}

		const blockedMessage = getImportedAudioPathsBlockedMessage();
		if (blockedMessage) {
			statusMessage = blockedMessage;
			return;
		}

		isBusy = true;
		try {
			activeJob = null;
			lastJob = null;
			statusMessage = 'Starting Audible acquisition.';
			const startedJob = (await tauriClient.startRemoteSourceAcquisition({
				providerId,
				selections: [...selectedTitleIds].map((titleId) => ({
					titleId,
					includeSupplementalPdf: includePdfByTitleId[titleId] ?? false,
				})),
			})) as AcquisitionJobWithProgress;
			activeJob = startedJob;
			lastJob = startedJob;
			statusMessage = statusFromAcquisitionJob(startedJob);
			await tick();
			const terminalJob = await pollAcquisitionToTerminal(startedJob);
			await finishAcquisitionJob(terminalJob);
		} catch (cause) {
			setError(cause, 'Failed to acquire selected Audible titles.');
		} finally {
			isBusy = false;
		}
	}

	async function cancelActiveAcquisition(): Promise<void> {
		if (!activeJob || acquisitionJobIsTerminal(activeJob)) return;
		try {
			const cancelledJob = (await tauriClient.cancelRemoteSourceAcquisition(
				activeJob.jobId,
			)) as AcquisitionJobWithProgress;
			activeJob = cancelledJob;
			lastJob = cancelledJob;
			statusMessage = statusFromAcquisitionJob(cancelledJob);
		} catch (cause) {
			setError(cause, 'Failed to cancel Audible acquisition.');
		}
	}

	$effect(() => {
		if (remoteSourceAcquireState.isOpen && !didHydrateOpenDialog) {
			didHydrateOpenDialog = true;
			void hydrateOpenDialog();
		}
		if (!remoteSourceAcquireState.isOpen) {
			didHydrateOpenDialog = false;
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
				{#if accountState?.status === 'connected'}
					<button class="btn-pill btn-pill-secondary" disabled={isBusy} onclick={() => void logout()}>
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

				{#if accountState?.status !== 'connected'}
					<div class="app-modal-field app-modal-field-button">
						<button class="btn-pill btn-pill-primary" disabled={isBusy} onclick={() => void startAuth()}>
							Connect Audible
						</button>
					</div>
					<div class="app-modal-field remote-source-handoff">
						<label for="remote-source-handoff">Auth Handoff</label>
						<input
							id="remote-source-handoff"
							type="text"
							placeholder="Final Amazon URL or handoff file path"
							bind:value={handoffPath}
						/>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<button class="btn-pill btn-pill-secondary" disabled={isBusy} onclick={() => void completeAuth()}>
							Complete Auth
						</button>
					</div>
				{:else}
					<div class="app-modal-field app-modal-field-button">
						<button class="btn-pill btn-pill-secondary" disabled={isBusy} onclick={() => void loadLibrary()}>
							Refresh Library
						</button>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<button
							class="btn-pill btn-pill-primary"
							disabled={isBusy || selectedTitleIds.size === 0}
							onclick={() => void acquireSelected()}
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
							bind:value={titleFilter}
						/>
					</div>
					<div class="app-modal-field app-modal-field-toggle remote-source-pdf-filter">
						<label class="checkbox-label text-xs mb-0">
							<input type="checkbox" bind:checked={showSupplementalPdfOnly} />
							<span class="option-label">Supplemental PDF only</span>
						</label>
					</div>
					<div class="app-modal-field app-modal-field-toggle remote-source-availability-filter">
						<label class="checkbox-label text-xs mb-0">
							<input type="checkbox" bind:checked={hideUnavailableTitles} />
							<span class="option-label">Hide unavailable</span>
						</label>
					</div>
				{/if}
			</div>

			{#if statusMessage}
				<div class="remote-source-status text-xs" aria-live="polite">{statusMessage}</div>
			{/if}

			{#if accountState?.status === 'connected'}
				<div class="remote-selection-summary" aria-live="polite">
					<span>{selectedTitleSummary()}</span>
					{#if selectedTitleIds.size > 0}
						<button
							class="remote-clear-selection"
							type="button"
							disabled={isBusy}
							onclick={clearSelectedTitles}
						>
							Clear selection
						</button>
					{/if}
				</div>
			{/if}

			{#if activeJob?.progress}
				<div class="remote-progress" role="status" aria-live="polite">
					<div
						class="remote-progress-bar"
						role="progressbar"
						aria-label="Acquisition progress"
						aria-valuemin="0"
						aria-valuemax="100"
						aria-valuenow={Math.round(progressPercent(activeJob))}
					>
						<span style={`width: ${progressPercent(activeJob)}%;`}></span>
					</div>
					<div class="remote-progress-copy">
						<span>{progressTitleLabel(activeJob.progress)}</span>
						<span>{Math.round(progressPercent(activeJob))}%</span>
					</div>
					{#if bytesLabel(activeJob.progress)}
						<p class="remote-progress-bytes">{bytesLabel(activeJob.progress)}</p>
					{/if}
					{#if !acquisitionJobIsTerminal(activeJob)}
						<button
							class="btn-pill btn-pill-secondary remote-progress-cancel"
							type="button"
							onclick={() => void cancelActiveAcquisition()}
						>
							Cancel Acquisition
						</button>
					{/if}
				</div>
			{/if}

			{#if accountState?.status === 'connected'}
				<div
					class="app-modal-results remote-title-list"
					role="listbox"
					aria-label="Audible titles"
					aria-multiselectable="true"
				>
					{#each filteredTitles() as title (title.titleId)}
						<div
							class="remote-title-row"
							class:selected={selectedTitleIds.has(title.titleId)}
							class:unavailable={!titleIsAcquirable(title)}
							role="option"
							aria-selected={selectedTitleIds.has(title.titleId)}
							aria-disabled={!titleIsAcquirable(title)}
						>
							<button
								type="button"
								class="remote-title-button"
								disabled={!titleIsAcquirable(title)}
								onclick={() => toggleTitle(title)}
							>
								<span class="remote-title-name">{title.title}</span>
								<span class="remote-title-meta">{title.authors.join(', ')}</span>
								{#if !titleIsAcquirable(title)}
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
										disabled={!titleIsAcquirable(title)}
										checked={includePdfByTitleId[title.titleId] ?? true}
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
