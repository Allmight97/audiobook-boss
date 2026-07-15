<script lang="ts">
	import type { Snippet } from 'svelte';
	import { PopoverController } from '../../lib/ui/popover.svelte';
	import { openAppSettingsDialog } from '../appSettings';
	import { handleClickToSelect, handleClickToSelectFolder } from '../fileImport';
	import {
		getSelectedFileIndices,
		openMetadataSurfaceForCurrentSelection,
		readFileListCount,
		removeSelectedFiles,
	} from '../fileList';
	import { handleMergeModeChange, JobControlsIsland } from '../jobControls';
	import { openRemoteSourceAcquire } from '../remoteSource';
	import { triggerProcessFromStatusPanel } from '../statusPanel';
	import { openMetadataLookup } from '../metadataLookup';
	import { OperationsBarIsland } from '../operationsBar';
	import { readEncoderSummaryLabel } from '../encoderPanel';
	import EncoderWorkbenchIsland from '../encoderPanel/EncoderWorkbenchIsland.svelte';
	import { OutputPanelIsland, readOutputNamingSummaryLabel } from '../outputPanel';
	import { TagPreviewIsland } from '../tagPreview';
	import { densityState, setDensityFromUser } from './density.svelte';

interface Props {
	children: Snippet;
	overlay?: Snippet;
}

let { children, overlay }: Props = $props();
	const selectedFileCount = $derived(getSelectedFileIndices().size);
	const fileCount = $derived(readFileListCount());
	const encoderSummaryLabel = $derived(readEncoderSummaryLabel());
	const namingSummaryLabel = $derived(readOutputNamingSummaryLabel());
	let importMenuOpen = $state(false);
	let importSplitElement = $state<HTMLElement | null>(null);
	let importCaretElement = $state<HTMLButtonElement | null>(null);
	let popoverContainer = $state<HTMLElement | null>(null);
	let encoderAnchor = $state<HTMLElement | null>(null);
	let encoderPanel = $state<HTMLElement | null>(null);
	let namingAnchor = $state<HTMLElement | null>(null);
	let namingPanel = $state<HTMLElement | null>(null);
	const encoderPopover = new PopoverController();
	const namingPopover = new PopoverController();

	$effect(() => {
		encoderPopover.setElements({ anchor: encoderAnchor, container: popoverContainer, panel: encoderPanel });
		namingPopover.setElements({ anchor: namingAnchor, container: popoverContainer, panel: namingPanel });
	});

	function togglePopover(kind: 'encoder' | 'naming'): void {
		const active = kind === 'encoder' ? encoderPopover : namingPopover;
		const other = kind === 'encoder' ? namingPopover : encoderPopover;
		other.close();
		active.toggle();
	}

	function handleWindowClick(event: MouseEvent): void {
		const target = event.target;
		if (!(target instanceof Node)) return;
		if (encoderPopover.isOpen && !encoderAnchor?.contains(target) && !encoderPanel?.contains(target)) {
			encoderPopover.close({ restoreFocus: false });
		}
		if (namingPopover.isOpen && !namingAnchor?.contains(target) && !namingPanel?.contains(target)) {
			namingPopover.close({ restoreFocus: false });
		}
		if (importMenuOpen && !importSplitElement?.contains(target)) {
			importMenuOpen = false;
		}
	}

	function handleImportMenuKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape' && importMenuOpen) {
			event.preventDefault();
			importMenuOpen = false;
			importCaretElement?.focus();
		}
	}

	function handleAddFolderSelect(): void {
		importMenuOpen = false;
		void handleClickToSelectFolder();
	}
</script>

<svelte:window onclick={handleWindowClick} />

<div class="app-shell" bind:this={popoverContainer}>
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
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div
				class="split-button"
				bind:this={importSplitElement}
				onkeydown={handleImportMenuKeydown}
			>
				<button
					id="import-files-btn"
					type="button"
					class="btn-pill btn-pill-secondary split-main"
					onclick={() => void handleClickToSelect()}
				>
					＋ Import
				</button>
				<button
					id="import-menu-toggle"
					type="button"
					class="btn-pill btn-pill-secondary split-caret"
					aria-haspopup="menu"
					aria-expanded={importMenuOpen}
					aria-label="More import options"
					bind:this={importCaretElement}
					onclick={() => {
						importMenuOpen = !importMenuOpen;
					}}
				>
					▼
				</button>
				<div class="split-dropdown" class:open={importMenuOpen} role="menu">
					<button
						id="add-folder-btn"
						type="button"
						class="split-option"
						role="menuitem"
						onclick={handleAddFolderSelect}
					>
						Add Folder
					</button>
				</div>
			</div>
			<button
				id="acquire-audiobooks-btn"
				type="button"
				class="btn-pill btn-pill-secondary"
				onclick={openRemoteSourceAcquire}
			>
				Import from Library
			</button>
			<div class="app-shell-merge" hidden={selectedFileCount > 0}>
				<JobControlsIsland {fileCount} onMergeModeChange={handleMergeModeChange} />
			</div>
		</div>
		{#if selectedFileCount > 0}
			<div class="app-shell-toolbar-selection" aria-label="Selected files actions">
				<span class="app-badge app-badge-info">{selectedFileCount} selected</span>
				<button
					type="button"
					class="btn-pill btn-pill-secondary"
					onclick={openMetadataLookup}
				>
					Find metadata ({selectedFileCount})
				</button>
				<button
					type="button"
					class="btn-pill btn-pill-secondary"
					disabled={selectedFileCount < 2}
					data-metadata-selection-intent
					onclick={() => void openMetadataSurfaceForCurrentSelection()}
				>
					Edit shared fields ({selectedFileCount})
				</button>
				<button
					type="button"
					class="btn-pill btn-pill-secondary"
					data-metadata-selection-intent
					onclick={() => void removeSelectedFiles()}
				>
					Remove
				</button>
			</div>
		{/if}
		<div class="app-shell-toolbar-end">
			<button
				bind:this={encoderAnchor}
				type="button"
				class="btn-pill btn-pill-secondary"
				data-testid="encoder-popover-trigger"
				aria-haspopup="dialog"
				aria-expanded={encoderPopover.isOpen}
				onclick={() => togglePopover('encoder')}
			>
				{encoderSummaryLabel}
			</button>
			<button
				bind:this={namingAnchor}
				type="button"
				class="btn-pill btn-pill-secondary"
				data-testid="naming-popover-trigger"
				aria-haspopup="dialog"
				aria-expanded={namingPopover.isOpen}
				onclick={() => togglePopover('naming')}
			>
				{namingSummaryLabel}
			</button>
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

	{#if encoderPopover.isOpen}
		<div
			bind:this={encoderPanel}
			class="app-popover app-shell-popover app-shell-encoder-popover"
			role="dialog"
			aria-label="Encoder settings"
			tabindex="-1"
			style={`left: ${encoderPopover.position.left}px; top: ${encoderPopover.position.top}px`}
			onkeydown={(event) => encoderPopover.handleKeydown(event)}
		>
			<EncoderWorkbenchIsland />
		</div>
	{/if}

	{#if namingPopover.isOpen}
		<div
			bind:this={namingPanel}
			class="app-popover app-shell-popover app-shell-naming-popover"
			role="dialog"
			aria-label="Output naming and tag preview"
			tabindex="-1"
			style={`left: ${namingPopover.position.left}px; top: ${namingPopover.position.top}px`}
			onkeydown={(event) => namingPopover.handleKeydown(event)}
		>
			<OutputPanelIsland variant="workbench" />
			<div class="app-shell-tag-preview">
				<h3>Tags Preview</h3>
				<TagPreviewIsland variant="workbench" />
			</div>
		</div>
	{/if}

	<main class="app-shell-main" data-testid="app-shell-main">
		{@render children()}
	</main>
	{@render overlay?.()}

	<div class="app-shell-operations" data-testid="app-shell-operations">
		<OperationsBarIsland />
	</div>
</div>

<style>
	.app-shell {
		position: relative;
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
		/* The primary action must never clip out of view at narrow widths. */
		flex-wrap: wrap;
		gap: var(--space-2) var(--space-3);
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

	.app-shell-merge {
		display: flex;
		align-items: center;
	}

	.app-shell-merge[hidden] {
		display: none;
	}

	.app-shell-main {
		display: flex;
		flex: 1 1 auto;
		min-height: 0;
		overflow: hidden;
	}

	.app-shell-popover {
		width: min(34rem, calc(100% - (2 * var(--space-3))));
		max-height: calc(100% - (2 * var(--space-3)));
		padding: var(--space-3);
		overflow: auto;
	}

	.app-shell-naming-popover {
		width: min(42rem, calc(100% - (2 * var(--space-3))));
	}

	.app-shell-tag-preview {
		margin-top: var(--space-3);
		padding-top: var(--space-3);
		border-top: 1px solid var(--border-primary);
	}

	.app-shell-tag-preview h3 {
		margin: 0 0 var(--space-2);
		font-size: var(--text-md);
	}
</style>
