<script lang="ts">
	import {
		EDITOR_TABS,
		type BookFixture,
		type EditorTabId,
	} from './fixtures';

	type Presentation = 'rail' | 'popover';

	type Props = {
		presentation: Presentation;
		popoverOpen?: boolean;
		popoverAnchor?: HTMLElement | null;
		shellWindowEl?: HTMLElement | null;
		selectedCount: number;
		multiMode: boolean;
		activeBook: BookFixture;
		selectedBooks: BookFixture[];
		editorTitle: string;
		editorSubtitle: string;
		editorGradient: string;
		onClosePopover?: () => void;
	};

	let {
		presentation,
		popoverOpen = false,
		popoverAnchor = null,
		shellWindowEl = null,
		selectedCount,
		multiMode,
		activeBook,
		selectedBooks,
		editorTitle,
		editorSubtitle,
		editorGradient,
		onClosePopover,
	}: Props = $props();

	const idPrefix = $derived(presentation === 'rail' ? 'rail' : 'pop');

	let activeTab = $state<EditorTabId>('metadata');
	let popoverEl = $state<HTMLElement | null>(null);
	let popTop = $state(0);
	let popLeft = $state(0);

	const tabIds = EDITOR_TABS.map((t) => t.id);

	function panelId(tab: EditorTabId): string {
		return `${idPrefix}-panel-${tab}`;
	}

	function tabId(tab: EditorTabId): string {
		return `${idPrefix}-tab-${tab}`;
	}

	function selectTab(tab: EditorTabId): void {
		activeTab = tab;
	}

	function handleTabKeydown(event: KeyboardEvent, tab: EditorTabId): void {
		const index = tabIds.indexOf(tab);
		if (index < 0) return;

		let next = index;
		if (event.key === 'ArrowRight') {
			next = (index + 1) % tabIds.length;
		} else if (event.key === 'ArrowLeft') {
			next = (index - 1 + tabIds.length) % tabIds.length;
		} else if (event.key === 'Home') {
			next = 0;
		} else if (event.key === 'End') {
			next = tabIds.length - 1;
		} else {
			return;
		}

		event.preventDefault();
		const nextTab = tabIds[next];
		activeTab = nextTab;
		document.getElementById(tabId(nextTab))?.focus();
	}

	function closePopover(): void {
		const restore = popoverAnchor?.querySelector('.proto-book-active') as HTMLElement | null;
		onClosePopover?.();
		queueMicrotask(() => {
			restore?.focus();
		});
	}

	function handlePopoverKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') {
			event.preventDefault();
			closePopover();
		}
	}

	$effect(() => {
		if (presentation !== 'popover' || !popoverOpen || !popoverAnchor || !shellWindowEl) {
			return;
		}

		const rowRect = popoverAnchor.getBoundingClientRect();
		const winRect = shellWindowEl.getBoundingClientRect();
		const popWidth = 330;
		const popHeight = 360;
		popTop = Math.min(rowRect.bottom - winRect.top + 6, Math.max(8, winRect.height - popHeight));
		popLeft = Math.min(
			Math.max(rowRect.left - winRect.left, 8),
			Math.max(8, winRect.width - popWidth - 8),
		);
	});

	$effect(() => {
		if (presentation !== 'popover' || !popoverOpen || !popoverEl) return;

		queueMicrotask(() => {
			const first = popoverEl?.querySelector<HTMLElement>(
				'input:not([disabled]), button:not([disabled]), [tabindex]:not([tabindex="-1"])',
			);
			first?.focus();
		});
	});
</script>

{#snippet metadataFields()}
	{#if multiMode}
		<div class="proto-batch-note" data-testid="editor-batch-note">
			Editing <b>{selectedCount} files</b>. Shared fields apply to every selected file on save;
			mixed values stay untouched unless you overwrite them.
		</div>
		<div class="proto-field">
			<label for="{idPrefix}-author">Author</label>
			<input id="{idPrefix}-author" type="text" placeholder="Mixed (3 values)" class="mixed" />
		</div>
		<div class="proto-field">
			<label for="{idPrefix}-series">Series</label>
			<input id="{idPrefix}-series" type="text" value="" />
		</div>
		<div class="proto-field">
			<label for="{idPrefix}-genre">Genre</label>
			<input id="{idPrefix}-genre" type="text" value="Science Fiction" />
		</div>
		<div class="proto-editor-actions">
			<button type="button" class="btn-pill btn-pill-primary" style="flex:1">
				Save to {selectedCount} files
			</button>
			<button type="button" class="btn-pill btn-pill-secondary">
				Find metadata ({selectedCount})
			</button>
		</div>
	{:else}
		<div class="proto-field">
			<label for="{idPrefix}-title-field">Book title</label>
			<input id="{idPrefix}-title-field" type="text" value={activeBook.title} />
		</div>
		<div class="proto-field">
			<label for="{idPrefix}-author-field">Author</label>
			<input id="{idPrefix}-author-field" type="text" value={activeBook.author} />
		</div>
		<div class="proto-field">
			<label for="{idPrefix}-narrator">Narrator</label>
			<input id="{idPrefix}-narrator" type="text" value={activeBook.narrator} />
		</div>
		<div class="proto-field-grid">
			<div class="proto-field">
				<label for="{idPrefix}-series-name">Series</label>
				<input id="{idPrefix}-series-name" type="text" value={activeBook.series} />
			</div>
			<div class="proto-field">
				<label for="{idPrefix}-book-num">Book #</label>
				<input id="{idPrefix}-book-num" type="text" value={activeBook.seriesNumber} />
			</div>
		</div>
		<div class="proto-editor-actions">
			<button type="button" class="btn-pill btn-pill-primary" style="flex:1">Save</button>
			<button type="button" class="btn-pill btn-pill-secondary">Find online</button>
		</div>
	{/if}
{/snippet}

{#snippet factsPanel()}
	{#if multiMode}
		<p class="proto-panel-lead muted-text">
			Showing facts for the active file ({activeBook.title}) in a {selectedCount}-file selection.
		</p>
	{/if}
	<dl class="proto-facts">
		<div><dt>Duration</dt><dd>{activeBook.duration}</dd></div>
		<div><dt>Size</dt><dd>{activeBook.size}</dd></div>
		<div><dt>Codec</dt><dd>{activeBook.codec}</dd></div>
		<div><dt>Bitrate</dt><dd>{activeBook.bitrate}</dd></div>
		<div><dt>Sample rate</dt><dd>{activeBook.sampleRate}</dd></div>
		<div><dt>Channels</dt><dd>{activeBook.channels}</dd></div>
		<div><dt>Chapters</dt><dd>{activeBook.chapters}</dd></div>
	</dl>
{/snippet}

{#snippet chaptersPanel()}
	<p class="proto-panel-lead muted-text">
		{activeBook.chapters} chapters · {multiMode ? `${selectedCount} files selected` : activeBook.title}
	</p>
	<ul class="proto-chapter-list">
		{#each activeBook.chapterTitles as chapter, index (chapter)}
			<li>
				<span class="proto-mono">{String(index + 1).padStart(2, '0')}</span>
				{chapter}
			</li>
		{/each}
		{#if activeBook.chapters > activeBook.chapterTitles.length}
			<li class="muted-text">…and {activeBook.chapters - activeBook.chapterTitles.length} more</li>
		{/if}
	</ul>
{/snippet}

{#snippet outputPanel()}
	{#if multiMode}
		<p class="proto-panel-lead muted-text">
			Output plan preview for {selectedCount} selected files. Per-file paths may differ.
		</p>
	{:else}
		<dl class="proto-facts">
			<div><dt>Format</dt><dd>{activeBook.outputFormat}</dd></div>
			<div><dt>Path</dt><dd class="proto-path">{activeBook.outputPath}</dd></div>
			<div><dt>Naming</dt><dd>{activeBook.outputNaming}</dd></div>
		</dl>
	{/if}
	<ul class="proto-output-list">
		{#each (multiMode ? selectedBooks : [activeBook]) as book (book.id)}
			<li>
				<span class="proto-output-title">{book.title}</span>
				<span class="proto-path muted-text">{book.outputPath}</span>
			</li>
		{/each}
	</ul>
{/snippet}

{#snippet editorBody()}
	<div class="proto-editor-tabs" role="tablist" aria-label="Metadata editor sections">
		{#each EDITOR_TABS as tab (tab.id)}
			<button
				id={tabId(tab.id)}
				type="button"
				role="tab"
				class="proto-editor-tab"
				class:on={activeTab === tab.id}
				aria-selected={activeTab === tab.id}
				tabindex={activeTab === tab.id ? 0 : -1}
				aria-controls={panelId(tab.id)}
				onclick={() => selectTab(tab.id)}
				onkeydown={(e) => handleTabKeydown(e, tab.id)}
			>
				{tab.label}
			</button>
		{/each}
	</div>
	{#each EDITOR_TABS as tab (tab.id)}
		{#if activeTab === tab.id}
			<div
				id={panelId(tab.id)}
				role="tabpanel"
				class="proto-editor-pane"
				aria-labelledby={tabId(tab.id)}
				data-testid={`editor-panel-${tab.id}`}
			>
				{#if tab.id === 'metadata'}
					{@render metadataFields()}
				{:else if tab.id === 'facts'}
					{@render factsPanel()}
				{:else if tab.id === 'chapters'}
					{@render chaptersPanel()}
				{:else}
					{@render outputPanel()}
				{/if}
			</div>
		{/if}
	{/each}
{/snippet}

{#if presentation === 'rail'}
	<aside class="proto-rail" data-testid="metadata-rail">
		<div class="proto-rail-head">
			<div
				class="app-cover-thumb proto-rail-cover"
				style={`background: ${editorGradient}`}
			></div>
			<div class="proto-rail-titles">
				<h2>{editorTitle}</h2>
				<p class="muted-text">{editorSubtitle}</p>
			</div>
		</div>
		{@render editorBody()}
	</aside>
{:else if popoverOpen}
	<button
		type="button"
		class="proto-pop-backdrop"
		aria-label="Dismiss metadata editor"
		onclick={closePopover}
	></button>
	<div
		bind:this={popoverEl}
		class="proto-pop open"
		role="dialog"
		aria-label="Metadata editor"
		aria-modal="true"
		tabindex="-1"
		style={`top: ${popTop}px; left: ${popLeft}px`}
		onkeydown={handlePopoverKeydown}
		data-testid="metadata-popover"
	>
		<div class="proto-pop-head">
			<div
				class="app-cover-thumb proto-pop-cover"
				style={`background: ${activeBook.gradient}`}
			></div>
			<span class="proto-pop-title">{editorTitle}</span>
			<button
				type="button"
				class="btn-pill btn-pill-secondary proto-btn-xs"
				aria-label="Close metadata editor"
				onclick={closePopover}
				data-testid="popover-close"
			>
				✕
			</button>
		</div>
		{@render editorBody()}
	</div>
{/if}

<style>
	.proto-rail {
		display: flex;
		flex-direction: column;
		min-width: 0;
		overflow: auto;
	}

	.proto-rail-head {
		display: flex;
		gap: var(--space-3);
		align-items: center;
		padding: var(--space-3) var(--space-4) 0;
	}

	.proto-rail-cover {
		--cover-thumb-size: 3.5rem;
		border: none;
	}

	.proto-rail-titles {
		min-width: 0;
		flex: 1;
	}

	.proto-rail-titles h2 {
		font-size: var(--text-lg);
		font-weight: 600;
		color: var(--text-primary);
		margin: 0;
	}

	.proto-rail-titles p {
		font-size: var(--text-sm);
		margin: 2px 0 0;
	}

	.proto-editor-tabs {
		display: flex;
		padding: var(--space-2) var(--space-3) 0;
		border-bottom: 1px solid var(--border-primary);
	}

	.proto-editor-tab {
		margin-top: 0;
		background: none;
		border: none;
		color: var(--text-muted);
		font-size: var(--text-sm);
		font-weight: 500;
		padding: 9px 10px;
		border-bottom: 2px solid transparent;
		border-radius: 0;
		cursor: pointer;
	}

	.proto-editor-tab.on {
		color: var(--text-primary);
		border-bottom-color: var(--accent-primary);
	}

	.proto-editor-pane {
		padding: var(--space-3) var(--space-4);
		display: flex;
		flex-direction: column;
		gap: var(--space-2);
	}

	.proto-batch-note {
		font-size: var(--text-sm);
		color: var(--text-muted);
		background: var(--bg-panel);
		border: 1px solid var(--border-primary);
		border-radius: var(--radius-md);
		padding: var(--space-2) var(--space-2);
		line-height: 1.5;
	}

	.proto-field label {
		display: block;
		font-size: var(--text-xs);
		font-weight: 500;
		color: var(--text-muted);
		margin-bottom: 4px;
	}

	.proto-field input {
		width: 100%;
		margin-top: 0;
		background: var(--bg-input);
		border: 1px solid transparent;
		border-radius: var(--radius-sm);
		color: var(--text-primary);
		font-size: var(--text-md);
		padding: 7px 9px;
	}

	.proto-field input:focus {
		outline: none;
		border-color: var(--border-focus);
	}

	.proto-field input.mixed {
		color: var(--text-placeholder);
		font-style: italic;
	}

	.proto-field-grid {
		display: grid;
		grid-template-columns: 1fr 5rem;
		gap: var(--space-2);
	}

	.proto-editor-actions {
		display: flex;
		gap: var(--space-2);
		margin-top: 2px;
	}

	.proto-panel-lead {
		font-size: var(--text-sm);
		margin: 0;
	}

	.proto-facts {
		display: grid;
		gap: var(--space-2);
		margin: 0;
	}

	.proto-facts div {
		display: grid;
		grid-template-columns: 6.5rem 1fr;
		gap: var(--space-2);
		font-size: var(--text-sm);
	}

	.proto-facts dt {
		color: var(--text-muted);
		margin: 0;
	}

	.proto-facts dd {
		margin: 0;
		color: var(--text-secondary);
	}

	.proto-path {
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		word-break: break-all;
	}

	.proto-chapter-list,
	.proto-output-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--space-1);
		font-size: var(--text-sm);
	}

	.proto-chapter-list li {
		display: flex;
		gap: var(--space-2);
		align-items: baseline;
	}

	.proto-output-list li {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: var(--space-2);
		border: 1px solid var(--border-primary);
		border-radius: var(--radius-sm);
	}

	.proto-output-title {
		font-weight: 500;
	}

	.proto-mono {
		font-family: var(--font-mono);
		color: var(--text-muted);
	}

	.proto-pop {
		position: absolute;
		width: 20.625rem;
		background: var(--bg-panel);
		border: 1px solid var(--border-secondary);
		border-radius: var(--radius-lg);
		box-shadow: var(--shadow-md);
		z-index: 21;
	}

	.proto-pop-backdrop {
		position: absolute;
		inset: 0;
		margin: 0;
		padding: 0;
		border: none;
		background: transparent;
		cursor: default;
		z-index: 20;
	}

	.proto-pop-head {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		padding: var(--space-3) var(--space-3);
		border-bottom: 1px solid var(--border-primary);
	}

	.proto-pop-cover {
		--cover-thumb-size: 2.125rem;
		border: none;
	}

	.proto-pop-title {
		font-size: var(--text-md);
		font-weight: 600;
		flex: 1;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.proto-btn-xs {
		padding: 3px 10px !important;
		font-size: var(--text-xs) !important;
	}
</style>
