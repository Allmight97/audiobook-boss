<script lang="ts">
	import { onMount } from 'svelte';
	import BookTable from './BookTable.svelte';
	import MetadataEditor from './MetadataEditor.svelte';
	import OperationsPanel, { type OpsMode } from './OperationsPanel.svelte';
	import {
		BOOK_FIXTURES,
		MIN_SHELL_WIDTH_REM,
		MULTI_SELECTION_IDS,
		type BookFixture,
	} from './fixtures';
	import { createPrototypeSelection } from './selection.svelte';

	type EditSurface = 'rail' | 'popover';
	type Density = 'comfortable' | 'compact';

	const selection = createPrototypeSelection({
		validIds: BOOK_FIXTURES.map((b) => b.id),
		initialIds: [0],
		initialActiveId: 0,
	});

	let surface = $state<EditSurface>('rail');
	let opsMode = $state<OpsMode>('pinned');
	let density = $state<Density>('comfortable');
	let popoverOpen = $state(false);
	let popoverAnchor = $state<HTMLTableRowElement | null>(null);
	let expandedOp = $state(0);
	let shellWindowEl = $state<HTMLElement | null>(null);

	const multiMode = $derived(selection.isMulti());
	const selectedCount = $derived(selection.count);
	const railVisible = $derived(surface === 'rail');

	const activeBook = $derived(
		BOOK_FIXTURES.find((b) => b.id === selection.activeId) ?? BOOK_FIXTURES[0],
	);

	const selectedBooks = $derived(
		BOOK_FIXTURES.filter((b) => selection.isSelected(b.id)),
	);

	const editorTitle = $derived(
		multiMode ? `${selectedCount} files selected` : activeBook.title,
	);

	const editorSubtitle = $derived(
		multiMode
			? selectedBooks.map((b) => b.title).join(' · ')
			: activeBook.subtitle,
	);

	const editorGradient = $derived(
		multiMode ? (selectedBooks[0]?.gradient ?? activeBook.gradient) : activeBook.gradient,
	);

	function setDensity(next: Density): void {
		density = next;
		if (next === 'compact') {
			document.documentElement.dataset.density = 'compact';
		} else {
			delete document.documentElement.dataset.density;
		}
	}

	function setSurface(next: EditSurface): void {
		surface = next;
		if (next === 'rail') {
			popoverOpen = false;
			popoverAnchor = null;
		} else if (selection.activeId !== null) {
			popoverOpen = true;
		}
	}

	function applySinglePreset(): void {
		const id = selection.activeId ?? 0;
		selection.apply({ type: 'replacePreset', ids: [id] });
	}

	function applyMultiPreset(): void {
		selection.apply({ type: 'replacePreset', ids: MULTI_SELECTION_IDS });
	}

	function handleActivateBook(_book: BookFixture, rowEl: HTMLTableRowElement): void {
		popoverAnchor = rowEl;
		if (surface === 'popover') {
			popoverOpen = true;
		}
	}

	function toggleExpandedOp(index: number): void {
		expandedOp = expandedOp === index ? -1 : index;
	}

	onMount(() => {
		setDensity(density);
	});
</script>

<div class="proto-chrome">
	<header class="proto-header">
		<h1>Direction v3 — the B1×B2 hybrid</h1>
		<p>Interactive shell using design tokens and primitives. Open forks as live toggles.</p>
	</header>

	<div class="proto-forks" role="group" aria-label="Open forks">
		<div class="proto-fork">
			<span>Edit surface</span>
			<div class="proto-seg">
				<button
					type="button"
					class:on={surface === 'rail'}
					aria-pressed={surface === 'rail'}
					onclick={() => setSurface('rail')}
				>
					Rail
				</button>
				<button
					type="button"
					class:on={surface === 'popover'}
					aria-pressed={surface === 'popover'}
					onclick={() => setSurface('popover')}
				>
					Popover
				</button>
			</div>
		</div>
		<div class="proto-fork">
			<span>Selection demo</span>
			<div class="proto-seg">
				<button
					type="button"
					class:on={!multiMode}
					aria-pressed={!multiMode}
					onclick={applySinglePreset}
					data-testid="fork-single"
				>
					Single file
				</button>
				<button
					type="button"
					class:on={multiMode && selectedCount === MULTI_SELECTION_IDS.length}
					aria-pressed={multiMode && selectedCount === MULTI_SELECTION_IDS.length}
					onclick={applyMultiPreset}
					data-testid="fork-multi"
				>
					3 files selected
				</button>
			</div>
		</div>
		<div class="proto-fork">
			<span>Ops panel</span>
			<div class="proto-seg">
				<button
					type="button"
					class:on={opsMode === 'collapsed'}
					aria-pressed={opsMode === 'collapsed'}
					onclick={() => {
						opsMode = 'collapsed';
					}}
					data-testid="fork-ops-collapsed"
				>
					Collapsed
				</button>
				<button
					type="button"
					class:on={opsMode === 'pinned'}
					aria-pressed={opsMode === 'pinned'}
					onclick={() => {
						opsMode = 'pinned';
					}}
					data-testid="fork-ops-pinned"
				>
					Pinned open
				</button>
			</div>
		</div>
	</div>

	<!-- Desktop scope: min width with horizontal overflow, not a mobile layout. -->
	<div
		bind:this={shellWindowEl}
		class="proto-window"
		class:compact={density === 'compact'}
		style={`min-width: ${MIN_SHELL_WIDTH_REM}rem`}
		data-testid="proto-window"
	>
		<div class="proto-appbar">
			<div class="proto-dots" aria-hidden="true">
				<div class="proto-dot" style="background:#ff5f57"></div>
				<div class="proto-dot" style="background:#febc2e"></div>
				<div class="proto-dot" style="background:#28c840"></div>
			</div>
			<span class="proto-app-title">Audiobook Boss</span>
			<button type="button" class="proto-nav-tab on">Process</button>
			<button type="button" class="proto-nav-tab">Settings</button>
			<div class="proto-density" role="group" aria-label="Density">
				<button
					type="button"
					class:on={density === 'comfortable'}
					aria-pressed={density === 'comfortable'}
					onclick={() => setDensity('comfortable')}
					data-testid="density-comfortable"
				>
					Comfortable
				</button>
				<button
					type="button"
					class:on={density === 'compact'}
					aria-pressed={density === 'compact'}
					onclick={() => setDensity('compact')}
					data-testid="density-compact"
				>
					Compact
				</button>
			</div>
		</div>

		<div class="proto-toolbar" class:multi={multiMode}>
			<button type="button" class="btn-pill btn-pill-secondary">＋ Import</button>
			<button type="button" class="btn-pill btn-pill-secondary">☁ Audible</button>
			<span class="proto-badge proto-badge-info proto-single-hint">merge — 3 files → one M4B</span>
			{#if multiMode}
				<div class="proto-sel-actions">
					<span class="proto-badge proto-badge-info" data-testid="selection-count">
						{selectedCount} selected
					</span>
					<button type="button" class="btn-pill btn-pill-secondary proto-btn-sm">
						Find metadata ({selectedCount})
					</button>
					<button type="button" class="btn-pill btn-pill-secondary proto-btn-sm">
						Edit shared fields
					</button>
					<button type="button" class="btn-pill btn-pill-secondary proto-btn-sm">Remove</button>
				</div>
			{/if}
			<span class="proto-toolbar-spacer"></span>
			<button type="button" class="btn-pill btn-pill-secondary">FDK HE-AAC · VBR 3 ▾</button>
			<button type="button" class="btn-pill btn-pill-secondary">ABS naming ▾</button>
			<button type="button" class="btn-pill btn-pill-primary">Process</button>
		</div>

		<div class="proto-main" class:no-rail={!railVisible}>
			<div class="proto-left">
				<BookTable {selection} {density} onActivateBook={handleActivateBook} />
				<OperationsPanel
					mode={opsMode}
					{expandedOp}
					onSetMode={(mode) => {
						opsMode = mode;
					}}
					onToggleExpanded={toggleExpandedOp}
				/>
			</div>

			{#if railVisible}
				<MetadataEditor
					presentation="rail"
					{selectedCount}
					{multiMode}
					{activeBook}
					{selectedBooks}
					editorTitle={editorTitle}
					editorSubtitle={editorSubtitle}
					editorGradient={editorGradient}
				/>
			{/if}
		</div>

		{#if surface === 'popover'}
			<MetadataEditor
				presentation="popover"
				{popoverOpen}
				{popoverAnchor}
				{shellWindowEl}
				{selectedCount}
				{multiMode}
				{activeBook}
				{selectedBooks}
				editorTitle={editorTitle}
				editorSubtitle={editorSubtitle}
				editorGradient={editorGradient}
				onClosePopover={() => {
					popoverOpen = false;
				}}
			/>
		{/if}
	</div>
</div>

<style>
	:global(body) {
		overflow: auto;
	}

	.proto-chrome {
		max-width: 90rem;
		margin: 0 auto;
		padding: var(--space-5) var(--space-5) var(--space-6);
		min-height: 100vh;
	}

	.proto-header {
		margin-bottom: var(--space-2);
	}

	.proto-header h1 {
		font-size: var(--text-xl);
		font-weight: 600;
		color: var(--text-primary);
		margin: 0;
	}

	.proto-header p {
		font-size: var(--text-md);
		color: var(--text-muted);
		margin: var(--space-1) 0 0;
	}

	.proto-forks {
		display: flex;
		gap: var(--space-4);
		margin: var(--space-3) 0 var(--space-4);
		font-size: var(--text-sm);
		color: var(--text-muted);
		flex-wrap: wrap;
	}

	.proto-fork {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.proto-seg {
		display: flex;
		background: var(--bg-panel);
		border: 1px solid var(--border-primary);
		border-radius: var(--radius-pill);
		padding: 2px;
	}

	.proto-seg button {
		margin-top: 0;
		border: none;
		background: transparent;
		color: var(--text-muted);
		font-size: var(--text-sm);
		font-weight: 500;
		padding: 4px 12px;
		border-radius: var(--radius-pill);
		cursor: pointer;
	}

	.proto-seg button.on {
		background: var(--accent-primary);
		color: #fff;
	}

	.proto-window {
		position: relative;
		display: flex;
		flex-direction: column;
		height: 55rem;
		background: var(--bg-main);
		border: 1px solid var(--border-primary);
		border-radius: var(--radius-lg);
		overflow: auto;
		box-shadow: var(--shadow-md);
	}

	.proto-appbar {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: 0 var(--space-3);
		height: 2.875rem;
		border-bottom: 1px solid var(--border-primary);
		flex-shrink: 0;
	}

	.proto-dots {
		display: flex;
		gap: 7px;
		margin-right: var(--space-2);
	}

	.proto-dot {
		width: 11px;
		height: 11px;
		border-radius: 50%;
	}

	.proto-app-title {
		font-size: var(--text-md);
		font-weight: 600;
		margin-right: var(--space-2);
	}

	.proto-nav-tab {
		margin-top: 0;
		background: none;
		border: none;
		color: var(--text-muted);
		font-size: var(--text-md);
		font-weight: 500;
		padding: 14px 12px;
		border-bottom: 2px solid transparent;
		border-radius: 0;
	}

	.proto-nav-tab.on {
		color: var(--text-primary);
		border-bottom-color: var(--accent-primary);
	}

	.proto-density {
		display: flex;
		background: var(--bg-input);
		border-radius: var(--radius-pill);
		padding: 2px;
		margin-left: auto;
	}

	.proto-density button {
		margin-top: 0;
		border: none;
		background: transparent;
		color: var(--text-muted);
		font-size: var(--text-sm);
		font-weight: 500;
		padding: 4px 12px;
		border-radius: var(--radius-pill);
	}

	.proto-density button.on {
		background: var(--accent-primary);
		color: #fff;
	}

	.proto-toolbar {
		display: flex;
		gap: var(--space-2);
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border-primary);
		align-items: center;
		flex-shrink: 0;
		flex-wrap: wrap;
	}

	.proto-sel-actions {
		display: flex;
		align-items: center;
		gap: var(--space-2);
	}

	.proto-toolbar.multi .proto-single-hint {
		display: none;
	}

	.proto-toolbar-spacer {
		flex: 1;
		min-width: var(--space-2);
	}

	.proto-btn-sm {
		padding: 5px 12px !important;
		font-size: var(--text-sm) !important;
	}

	.proto-badge {
		font-size: var(--text-xs);
		font-weight: 600;
		letter-spacing: 0.04em;
		text-transform: uppercase;
		padding: 2px 8px;
		border-radius: var(--radius-pill);
		white-space: nowrap;
	}

	.proto-badge-info {
		background: color-mix(in srgb, var(--accent-primary) 16%, transparent);
		color: var(--accent-primary-hover);
	}

	.proto-main {
		display: grid;
		grid-template-columns: 1fr 21.25rem;
		min-height: 0;
		flex: 1;
	}

	.proto-main.no-rail {
		grid-template-columns: 1fr;
	}

	.proto-left {
		display: flex;
		flex-direction: column;
		min-width: 0;
		border-right: 1px solid var(--border-primary);
	}

	.proto-main.no-rail .proto-left {
		border-right: none;
	}
</style>
