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
	import { openMetadataLookup } from '../metadataLookup';
	import { OperationsBarIsland } from '../operationsBar';
	import { ProcessSplitButton } from '../previewAudio';
	import { readEncoderSummaryLabel } from '../encoderPanel';
	import EncoderWorkbenchIsland from '../encoderPanel/EncoderWorkbenchIsland.svelte';
	import { OutputPanelIsland, readOutputNamingSummaryLabel } from '../outputPanel';
	import { TagPreviewIsland } from '../tagPreview';
	import { editSurfaceState } from '../metadataSurface';
	import { densityState, setDensityFromUser } from './density.svelte';
	import {
		RAIL_WIDTH_KEYBOARD_STEP,
		RAIL_WIDTH_MAX,
		RAIL_WIDTH_MIN,
		previewRailWidthFromUser,
		readRailWidth,
		setRailWidthFromUser,
	} from './railWidth.svelte';

interface Props {
	children: Snippet;
	overlay?: Snippet;
	rail?: Snippet;
}

let { children, overlay, rail }: Props = $props();
	const noRail = $derived(editSurfaceState.preference !== 'rail');
	const selectedFileCount = $derived(getSelectedFileIndices().size);
	const fileCount = $derived(readFileListCount());
	const encoderSummaryLabel = $derived(readEncoderSummaryLabel());
	const namingSummaryLabel = $derived(readOutputNamingSummaryLabel());
	let importMenuRoot = $state<HTMLElement | null>(null);
	let importMenuPanel = $state<HTMLElement | null>(null);
	let importPillElement = $state<HTMLButtonElement | null>(null);
	let popoverContainer = $state<HTMLElement | null>(null);
	let encoderAnchor = $state<HTMLElement | null>(null);
	let encoderPanel = $state<HTMLElement | null>(null);
	let namingAnchor = $state<HTMLElement | null>(null);
	let namingPanel = $state<HTMLElement | null>(null);
	const encoderPopover = new PopoverController();
	const namingPopover = new PopoverController();
	const importMenu = new PopoverController({ closeOnClickAway: true });
	const railWidth = $derived(readRailWidth());

	function handleRailResizePointerDown(event: PointerEvent): void {
		if (event.button !== 0) return;
		event.preventDefault();
		const pointerId = event.pointerId;
		const startX = event.clientX;
		const startWidth = readRailWidth();

		const handleMove = (moveEvent: PointerEvent): void => {
			if (moveEvent.pointerId !== pointerId) return;
			// The rail sits on the right: dragging left grows it.
			previewRailWidthFromUser(startWidth + (startX - moveEvent.clientX));
		};
		const finish = (endEvent: PointerEvent, commit: boolean): void => {
			if (endEvent.pointerId !== pointerId) return;
			window.removeEventListener('pointermove', handleMove);
			window.removeEventListener('pointerup', handleUp);
			window.removeEventListener('pointercancel', handleCancel);
			if (commit) setRailWidthFromUser(readRailWidth());
			else previewRailWidthFromUser(startWidth);
		};
		const handleUp = (endEvent: PointerEvent): void => finish(endEvent, true);
		const handleCancel = (endEvent: PointerEvent): void => finish(endEvent, false);

		const target = event.currentTarget;
		if (target instanceof Element && typeof target.setPointerCapture === 'function') {
			try {
				target.setPointerCapture(pointerId);
			} catch {
				// Best-effort; the window listeners carry the drag regardless.
			}
		}
		window.addEventListener('pointermove', handleMove);
		window.addEventListener('pointerup', handleUp);
		window.addEventListener('pointercancel', handleCancel);
	}

	function handleRailResizeKeydown(event: KeyboardEvent): void {
		const current = readRailWidth();
		let next: number | null = null;
		if (event.key === 'ArrowLeft') next = current + RAIL_WIDTH_KEYBOARD_STEP;
		else if (event.key === 'ArrowRight') next = current - RAIL_WIDTH_KEYBOARD_STEP;
		else if (event.key === 'Home') next = RAIL_WIDTH_MAX;
		else if (event.key === 'End') next = RAIL_WIDTH_MIN;
		if (next === null) return;
		event.preventDefault();
		setRailWidthFromUser(next);
	}

	$effect(() => {
		encoderPopover.setElements({ anchor: encoderAnchor, container: popoverContainer, panel: encoderPanel });
		namingPopover.setElements({ anchor: namingAnchor, container: popoverContainer, panel: namingPanel });
		importMenu.setElements({
			anchor: importPillElement,
			panel: importMenuPanel,
			clickBoundary: importMenuRoot,
		});
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
		importMenu.handleClickAway(event);
	}

	function handleImportMenuKeydown(event: KeyboardEvent): void {
		importMenu.handleKeydown(event);
	}

	function toggleImportMenuFromKeyboard(event: KeyboardEvent): void {
		if (!['Enter', ' ', 'ArrowDown'].includes(event.key)) return;
		event.preventDefault();
		importMenu.toggle({ focusInside: true });
	}

	function handleImportFilesSelect(): void {
		importMenu.close({ restoreFocus: false });
		void handleClickToSelect();
	}

	function handleAddFolderSelect(): void {
		importMenu.close({ restoreFocus: false });
		void handleClickToSelectFolder();
	}
</script>

<svelte:window onclick={handleWindowClick} />

<div class="app-shell" bind:this={popoverContainer}>
	<!-- data-tauri-drag-region="deep": with titleBarStyle Overlay the appbar
	     is the window's drag surface. "deep" makes the whole subtree drag
	     (tauri's drag.js walks the event path and lets clickable elements —
	     buttons, inputs — block dragging on their own), so nested tabs and
	     controls keep their clicks without per-element attributes. -->
	<header class="app-shell-appbar" data-testid="app-shell-appbar" data-tauri-drag-region="deep">
		<span class="app-shell-title">Audiobook Boss</span>
		<div class="tab-strip">
			<button type="button" class="tab on" aria-current="page">Process</button>
			<button type="button" class="tab" onclick={() => void openAppSettingsDialog()}>
				Settings
			</button>
		</div>
		<div class="segmented" role="group" aria-label="Density">
			<button
				type="button"
				aria-pressed={densityState.preference === 'comfortable'}
				onclick={() => setDensityFromUser('comfortable')}
			>
				Comfortable
			</button>
			<button
				type="button"
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
				bind:this={importMenuRoot}
				onkeydown={handleImportMenuKeydown}
			>
				<button
					id="import-files-btn"
					type="button"
					class="pill pill-ghost"
					aria-haspopup="menu"
					aria-expanded={importMenu.isOpen}
					bind:this={importPillElement}
					onclick={() => importMenu.toggle({ focusInside: false })}
					onkeydown={toggleImportMenuFromKeyboard}
				>
					＋ Import
				</button>
				<div
					bind:this={importMenuPanel}
					class="split-dropdown import-menu-dropdown"
					class:open={importMenu.isOpen}
					role="menu"
				>
					<button
						id="import-files-option"
						type="button"
						class="split-option"
						role="menuitem"
						onclick={handleImportFilesSelect}
					>
						Files…
					</button>
					<button
						id="add-folder-btn"
						type="button"
						class="split-option"
						role="menuitem"
						onclick={handleAddFolderSelect}
					>
						Folder…
					</button>
				</div>
			</div>
			<button
				id="acquire-audiobooks-btn"
				type="button"
				class="pill pill-ghost"
				onclick={openRemoteSourceAcquire}
			>
				☁ Audible
			</button>
			<div class="app-shell-merge">
				<JobControlsIsland {fileCount} onMergeModeChange={handleMergeModeChange} />
			</div>
		</div>
		{#if selectedFileCount > 0}
			<div class="app-shell-toolbar-selection" aria-label="Selected files actions">
				<span class="app-badge app-badge-info">{selectedFileCount} selected</span>
				<button type="button" class="pill pill-ghost pill-sm" onclick={openMetadataLookup}>
					Find metadata ({selectedFileCount})
				</button>
				<button
					type="button"
					class="pill pill-ghost pill-sm"
					disabled={selectedFileCount < 2}
					data-metadata-selection-intent
					onclick={() => void openMetadataSurfaceForCurrentSelection()}
				>
					Edit shared fields ({selectedFileCount})
				</button>
				<button
					type="button"
					class="pill pill-ghost pill-sm"
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
				class="pill pill-ghost"
				data-testid="encoder-popover-trigger"
				aria-haspopup="dialog"
				aria-expanded={encoderPopover.isOpen}
				onclick={() => togglePopover('encoder')}
			>
				{encoderSummaryLabel} ▾
			</button>
			<button
				bind:this={namingAnchor}
				type="button"
				class="pill pill-ghost"
				data-testid="naming-popover-trigger"
				aria-haspopup="dialog"
				aria-expanded={namingPopover.isOpen}
				onclick={() => togglePopover('naming')}
			>
				{namingSummaryLabel} ▾
			</button>
			<ProcessSplitButton />
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

	<main
		class="app-shell-main"
		class:no-rail={noRail}
		data-testid="app-shell-main"
		style={`--abb-rail-width: ${railWidth}px`}
	>
		<div class="app-shell-main-left">
			<div class="app-shell-main-content">
				{@render children()}
			</div>
			<div class="app-shell-operations" data-testid="app-shell-operations">
				<OperationsBarIsland />
			</div>
		</div>
		{#if rail && !noRail}
			<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
			<div
				class="app-shell-rail-resizer"
				role="separator"
				aria-orientation="vertical"
				aria-label="Resize metadata rail"
				aria-valuemin={RAIL_WIDTH_MIN}
				aria-valuemax={RAIL_WIDTH_MAX}
				aria-valuenow={railWidth}
				tabindex="0"
				onpointerdown={handleRailResizePointerDown}
				onkeydown={handleRailResizeKeydown}
			></div>
			<aside class="app-shell-main-rail">
				{@render rail()}
			</aside>
		{/if}
	</main>
	{@render overlay?.()}
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
	.app-shell-toolbar-end {
		display: flex;
		align-items: center;
	}

	.app-shell-appbar {
		height: var(--appbar-h);
		gap: var(--space-2);
		/* Left padding reserves space for macOS's overlay-titlebar traffic
		   lights (tauri.conf.json `titleBarStyle: "Overlay"`). Frontend code
		   has no platform read today, so this is unconditional; the config
		   flag only takes effect on macOS (tauri-runtime-wry gates it behind
		   `cfg(target_os = "macos")`), so this is dead space elsewhere. The
		   planned Linux port keeps native decorations and can drop this then. */
		padding: 0 var(--density-pad) 0 4.875rem;
		border-bottom: 1px solid var(--border-primary);
		background: var(--bg-main);
		flex-shrink: 0;
	}

	/* Mock .nav-tab (13px, 14x12) is larger than the rail-tab default. */
	.app-shell-appbar :global(.tab) {
		padding: 0.875rem 0.75rem;
		font-size: var(--text-md);
	}

	.app-shell-title {
		margin-right: var(--space-2);
		font-size: var(--text-md);
		font-weight: 600;
		color: var(--text-primary);
	}

	.segmented {
		margin-left: auto;
	}

	.app-shell-toolbar {
		justify-content: space-between;
		/* The primary action must never clip out of view at narrow widths. */
		flex-wrap: wrap;
		gap: 0.5rem;
		padding: 0.625rem 0.875rem;
		border-bottom: 1px solid var(--border-primary);
		background: var(--bg-main);
		flex-shrink: 0;
	}

	/* The import menu drops under a left-anchored pill. */
	.import-menu-dropdown {
		left: 0;
		right: auto;
		min-width: 8rem;
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

	.app-shell-main {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto var(--abb-rail-width, 420px);
		flex: 1 1 auto;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
	}

	.app-shell-main.no-rail {
		grid-template-columns: minmax(0, 1fr);
	}

	.app-shell-rail-resizer {
		width: 6px;
		margin-left: -3px;
		cursor: col-resize;
		touch-action: none;
	}

	.app-shell-rail-resizer:hover,
	.app-shell-rail-resizer:focus-visible {
		background: color-mix(in srgb, var(--accent-primary) 45%, transparent);
		outline: none;
	}

	.app-shell-main-left {
		display: flex;
		flex-direction: column;
		min-width: 0;
		min-height: 0;
		border-right: 1px solid var(--border-primary);
	}

	.app-shell-main.no-rail .app-shell-main-left {
		border-right: none;
	}

	.app-shell-main-content {
		display: flex;
		flex: 1 1 auto;
		min-height: 0;
		overflow: hidden;
	}

	/* The rail cell must never dictate the grid row height: without
	   min-height: 0 a tall Metadata/Chapters pane stretches the row past the
	   viewport and carries the left cell's operations bar off-screen. The
	   rail island scrolls internally (mock .rail { overflow: auto }). */
	.app-shell-main-rail {
		display: flex;
		flex-direction: column;
		min-width: 0;
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
