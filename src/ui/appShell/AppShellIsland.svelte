<script lang="ts">
	import type { Snippet } from 'svelte';
	import { openAppSettingsDialog } from '../appSettings';
	import { handleClickToSelect, handleClickToSelectFolder } from '../fileImport';
	import { getSelectedFileIndices, removeSelectedFiles } from '../fileList';
	import {
		handleMaxConcurrentSelectionChange,
		handleMergeModeChange,
		JobControlsIsland,
	} from '../jobControls';
	import { openRemoteSourceAcquire } from '../remoteSource';
	import { triggerProcessFromStatusPanel } from '../statusPanel';
	import { openMetadataLookup } from '../metadataLookup';
	import { densityState, setDensityFromUser } from './density.svelte';

	interface Props {
		children: Snippet;
	}

	let { children }: Props = $props();
	const selectedFileCount = $derived(getSelectedFileIndices().size);
</script>

<div class="app-shell">
	<header class="app-shell-appbar" data-testid="app-shell-appbar">
		<span class="app-shell-title">Audiobook Boss</span>
		<span class="app-shell-current-view" aria-current="page">Process</span>
		<button
			type="button"
			class="app-shell-settings"
			onclick={() => void openAppSettingsDialog()}
		>
			Settings
		</button>
		<div class="app-shell-density" role="group" aria-label="Density">
			<button
				type="button"
				class:active={densityState.preference === 'comfortable'}
				aria-pressed={densityState.preference === 'comfortable'}
				onclick={() => setDensityFromUser('comfortable')}
			>
				Comfortable
			</button>
			<button
				type="button"
				class:active={densityState.preference === 'compact'}
				aria-pressed={densityState.preference === 'compact'}
				onclick={() => setDensityFromUser('compact')}
			>
				Compact
			</button>
		</div>
	</header>

	<div class="app-shell-toolbar" data-testid="app-shell-toolbar">
		<div class="app-shell-toolbar-start">
			<button
				id="import-files-btn"
				type="button"
				class="btn-pill btn-pill-secondary"
				onclick={() => void handleClickToSelect()}
			>
				Import
			</button>
			<button
				id="add-folder-btn"
				type="button"
				class="btn-pill btn-pill-secondary"
				onclick={() => void handleClickToSelectFolder()}
			>
				Add Folder
			</button>
			<button
				id="acquire-audiobooks-btn"
				type="button"
				class="btn-pill btn-pill-secondary"
				onclick={openRemoteSourceAcquire}
			>
				Import from Library
			</button>
			<JobControlsIsland
				onMergeModeChange={handleMergeModeChange}
				onMaxConcurrentSelectionChange={handleMaxConcurrentSelectionChange}
			/>
		</div>
		<div class="app-shell-toolbar-selection" aria-label="Selected files actions">
			<span class="app-shell-toolbar-selection-count">{selectedFileCount} selected</span>
			<button
				type="button"
				class="btn-pill btn-pill-secondary"
				disabled={selectedFileCount === 0}
				onclick={openMetadataLookup}
			>
				Find metadata ({selectedFileCount})
			</button>
			<button
				type="button"
				class="btn-pill btn-pill-secondary"
				disabled={selectedFileCount === 0}
				onclick={() => void removeSelectedFiles()}
			>
				Remove
			</button>
		</div>
		<div class="app-shell-toolbar-end">
			<button
				id="process-button"
				type="button"
				class="btn-pill btn-pill-primary"
				onclick={() => triggerProcessFromStatusPanel()}
			>
				Process
			</button>
		</div>
	</div>

	<main class="app-shell-main" data-testid="app-shell-main">
		{@render children()}
	</main>
</div>

<style>
	.app-shell {
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		min-height: 0;
	}

	.app-shell-appbar,
	.app-shell-toolbar,
	.app-shell-toolbar-start,
	.app-shell-toolbar-selection,
	.app-shell-toolbar-end,
	.app-shell-density {
		display: flex;
		align-items: center;
	}

	.app-shell-appbar {
		min-height: var(--density-row-h);
		gap: var(--space-2);
		padding: 0 var(--density-pad);
		border-bottom: 1px solid var(--border-primary);
		background: var(--bg-panel);
		flex-shrink: 0;
	}

	.app-shell-title {
		font-size: var(--text-lg);
		font-weight: 650;
		color: var(--text-primary);
	}

	.app-shell-current-view,
	.app-shell-settings {
		margin-top: 0;
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-sm);
		background: transparent;
		color: var(--text-secondary);
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.app-shell-current-view {
		color: var(--text-primary);
	}

	.app-shell-settings:hover,
	.app-shell-settings:focus-visible {
		background: var(--bg-hover);
		color: var(--text-primary);
	}

	.app-shell-density {
		margin-left: auto;
		padding: 2px;
		border: 1px solid var(--border-primary);
		border-radius: var(--radius-pill);
		background: var(--bg-input);
	}

	.app-shell-density button {
		margin-top: 0;
		padding: var(--space-1) var(--space-2);
		border-radius: var(--radius-pill);
		background: transparent;
		color: var(--text-muted);
		font-size: var(--text-sm);
		font-weight: 500;
	}

	.app-shell-density button.active {
		background: var(--accent-primary);
		color: var(--text-inverse);
	}

	.app-shell-toolbar {
		justify-content: space-between;
		gap: var(--space-3);
		min-height: var(--density-row-h);
		padding: var(--space-2) var(--density-pad);
		border-bottom: 1px solid var(--border-primary);
		background: var(--bg-main);
		flex-shrink: 0;
	}

	.app-shell-toolbar-start,
	.app-shell-toolbar-selection,
	.app-shell-toolbar-end {
		gap: var(--space-2);
	}

	.app-shell-toolbar-selection {
		margin-left: auto;
	}

	.app-shell-toolbar-selection-count {
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.app-shell-main {
		display: flex;
		flex: 1 1 auto;
		min-height: 0;
		overflow: hidden;
	}
</style>
