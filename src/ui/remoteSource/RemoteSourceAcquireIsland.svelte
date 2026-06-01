<script lang="ts">
	import { tauriClient } from '../../lib/tauri/client';
	import type { AcquisitionJob, ProviderId, RemoteSourceAccountState, RemoteTitle } from '../../types/remoteSource';
	import { handleImportedAudioPaths } from '../fileImport/handlers';
	import { getCurrentFileList } from '../fileList/state.svelte';
	import { registerRemoteSourceSupplementalAssets } from './sessionAssets.svelte';

	const providerId: ProviderId = 'audible';

	let isOpen = $state(false);
	let isBusy = $state(false);
	let accountState = $state<RemoteSourceAccountState | null>(null);
	let titles = $state<RemoteTitle[]>([]);
	let selectedTitleIds = $state<Set<string>>(new Set());
	let includePdfByTitleId = $state<Record<string, boolean>>({});
	let titleFilter = $state('');
	let handoffPath = $state('');
	let statusMessage = $state('');
	let lastJob = $state<AcquisitionJob | null>(null);

	function setError(cause: unknown, fallback: string): void {
		console.error(fallback, cause);
		statusMessage = cause instanceof Error ? cause.message : fallback;
	}

	async function refreshAccountState(): Promise<void> {
		accountState = await tauriClient.getRemoteSourceAccountState(providerId);
	}

	async function toggleOpen(): Promise<void> {
		isOpen = !isOpen;
		if (!isOpen) return;
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
			includePdfByTitleId = Object.fromEntries(
				library.titles.map((title) => [title.titleId, title.supplementalPdfAvailable]),
			);
			statusMessage =
				library.diagnostics.length > 0
					? library.diagnostics.map((diagnostic) => diagnostic.message).join(' ')
					: `${library.titles.length} Audible titles loaded.`;
		} catch (cause) {
			setError(cause, 'Failed to load Audible library.');
		} finally {
			isBusy = false;
		}
	}

	function toggleTitle(title: RemoteTitle): void {
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

	function filteredTitles(): RemoteTitle[] {
		const normalizedFilter = titleFilter.trim().toLowerCase();
		if (!normalizedFilter) return titles;
		return titles.filter((title) =>
			[title.title, title.authors.join(' '), title.narrators.join(' ')]
				.join(' ')
				.toLowerCase()
				.includes(normalizedFilter),
		);
	}

	async function acquireSelected(): Promise<void> {
		if (selectedTitleIds.size === 0) {
			statusMessage = 'Select at least one Audible title.';
			return;
		}

		isBusy = true;
		try {
			lastJob = await tauriClient.startRemoteSourceAcquisition({
				providerId,
				selections: [...selectedTitleIds].map((titleId) => ({
					titleId,
					includeSupplementalPdf: includePdfByTitleId[titleId] ?? false,
				})),
			});
			const materializedPaths = lastJob.materializedFiles.map((file) => file.path);
			if (materializedPaths.length > 0) {
				await handleImportedAudioPaths(materializedPaths);
				registerRemoteSourceSupplementalAssets(lastJob, getCurrentFileList());
				statusMessage = `${materializedPaths.length} acquired title${materializedPaths.length === 1 ? '' : 's'} imported.`;
			} else {
				statusMessage =
					lastJob.diagnostics.map((diagnostic) => diagnostic.message).join(' ') ||
					'Audible acquisition did not materialize an importable file.';
			}
		} catch (cause) {
			setError(cause, 'Failed to acquire selected Audible titles.');
		} finally {
			isBusy = false;
		}
	}
</script>

<div class="remote-source">
	<button id="acquire-audiobooks-btn" class="btn-pill btn-pill-secondary" onclick={() => void toggleOpen()}>
		Acquire
	</button>

	{#if isOpen}
		<div class="remote-source-panel" aria-label="Acquire Audiobooks">
			<div class="section-header justify-between">
				<h3>Acquire Audiobooks</h3>
				{#if accountState?.status === 'connected'}
					<button class="btn-pill btn-pill-secondary" disabled={isBusy} onclick={() => void logout()}>
						Logout
					</button>
				{/if}
			</div>

			{#if statusMessage}
				<p class="text-xs muted-text" aria-live="polite">{statusMessage}</p>
			{/if}

			{#if accountState?.status !== 'connected'}
				<div class="remote-source-auth">
					<button class="btn-pill btn-pill-primary" disabled={isBusy} onclick={() => void startAuth()}>
						Connect Audible
					</button>
					<input
						class="input-field"
						type="text"
						placeholder="Auth response handoff file path"
						bind:value={handoffPath}
					/>
					<button class="btn-pill btn-pill-secondary" disabled={isBusy} onclick={() => void completeAuth()}>
						Complete Auth
					</button>
				</div>
			{:else}
				<div class="remote-source-actions">
					<button class="btn-pill btn-pill-secondary" disabled={isBusy} onclick={() => void loadLibrary()}>
						Refresh Library
					</button>
					<button class="btn-pill btn-pill-primary" disabled={isBusy} onclick={() => void acquireSelected()}>
						Acquire Selected
					</button>
				</div>

				<input
					class="input-field"
					type="search"
					placeholder="Filter loaded titles"
					bind:value={titleFilter}
				/>

				<div class="remote-title-list" role="listbox" aria-label="Audible titles" aria-multiselectable="true">
					{#each filteredTitles() as title (title.titleId)}
						<div
							class="remote-title-row"
							class:selected={selectedTitleIds.has(title.titleId)}
							role="option"
							aria-selected={selectedTitleIds.has(title.titleId)}
						>
							<button
								type="button"
								class="remote-title-button"
								onclick={() => toggleTitle(title)}
							>
								<span class="remote-title-name">{title.title}</span>
								<span class="remote-title-meta">{title.authors.join(', ')}</span>
							</button>
							{#if title.supplementalPdfAvailable}
								<label class="remote-pdf-toggle">
									<input
										type="checkbox"
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
	{/if}
</div>

<style>
	.remote-source {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.remote-source-panel {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		padding: 0.75rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.375rem;
		background-color: var(--bg-input);
	}

	.remote-source-auth,
	.remote-source-actions {
		display: flex;
		align-items: center;
		gap: 0.5rem;
	}

	.input-field {
		min-width: 0;
		flex: 1;
		padding: 0.35rem 0.5rem;
		border: 1px solid var(--border-primary);
		border-radius: 0.25rem;
		background-color: var(--bg-primary);
		color: var(--text-primary);
		font-size: 0.75rem;
	}

	.remote-title-list {
		display: flex;
		max-height: 14rem;
		flex-direction: column;
		overflow-y: auto;
		border: 1px solid var(--border-primary);
		border-radius: 0.25rem;
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

	.remote-title-button {
		display: flex;
		min-width: 0;
		flex: 1;
		flex-direction: column;
		gap: 0.125rem;
		padding: 0.5rem;
		border: 0;
		background: transparent;
		color: inherit;
		text-align: left;
		cursor: pointer;
	}

	.remote-title-name,
	.remote-title-meta {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.remote-title-name {
		font-size: 0.8rem;
		font-weight: 600;
	}

	.remote-title-meta,
	.remote-pdf-toggle {
		color: var(--text-muted);
		font-size: 0.72rem;
	}

	.remote-pdf-toggle {
		display: flex;
		align-items: center;
		gap: 0.25rem;
		padding-right: 0.5rem;
		white-space: nowrap;
	}
</style>
