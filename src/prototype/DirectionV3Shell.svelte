<script lang="ts">
	import { onMount } from 'svelte';
	import {
		BOOK_FIXTURES,
		MULTI_SELECTION_IDS,
		STATUS_BADGE,
		type BookFixture,
	} from './fixtures';

	type EditSurface = 'rail' | 'popover';
	type SelectionDemo = 'single' | 'multi';
	type Density = 'comfortable' | 'compact';

	let surface = $state<EditSurface>('rail');
	let selectionDemo = $state<SelectionDemo>('single');
	let opsOpen = $state(true);
	let opsPinned = $state(true);
	let density = $state<Density>('comfortable');
	let selectedIdx = $state(0);
	let checkedIds = $state<Set<number>>(new Set([0]));
	let popoverOpen = $state(false);
	let activeRailTab = $state('Metadata');
	let expandedOp = $state(0);

	const selectedBook = $derived(BOOK_FIXTURES[selectedIdx] ?? BOOK_FIXTURES[0]);
	const multiMode = $derived(selectionDemo === 'multi' || checkedIds.size > 1);
	const selectedCount = $derived(checkedIds.size);
	const railVisible = $derived(surface === 'rail');

	const railTitle = $derived(
		multiMode ? `${selectedCount} files selected` : selectedBook.title,
	);
	const railSubtitle = $derived(
		multiMode
			? 'A Change of Plans · The Future · The Martian'
			: selectedBook.subtitle,
	);
	const railGradient = $derived(
		multiMode ? BOOK_FIXTURES[1].gradient : selectedBook.gradient,
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
		} else {
			popoverOpen = true;
		}
	}

	function setSelectionDemo(next: SelectionDemo): void {
		selectionDemo = next;
		if (next === 'multi') {
			checkedIds = new Set(MULTI_SELECTION_IDS);
		} else {
			checkedIds = new Set([selectedIdx]);
		}
	}

	function isRowSelected(id: number): boolean {
		return checkedIds.has(id);
	}

	function selectRow(book: BookFixture, event: MouseEvent): void {
		const target = event.target as HTMLElement;
		if (target.closest('input[type="checkbox"]')) {
			return;
		}
		selectedIdx = book.id;
		if (selectionDemo === 'single') {
			checkedIds = new Set([book.id]);
		}
		if (surface === 'popover') {
			popoverOpen = true;
		}
	}

	function toggleRowCheck(book: BookFixture, checked: boolean): void {
		const next = new Set(checkedIds);
		if (checked) {
			next.add(book.id);
		} else {
			next.delete(book.id);
		}
		checkedIds = next;
		if (next.size === 1) {
			const only = [...next][0];
			selectedIdx = only;
		}
	}

	function toggleSelectAll(checked: boolean): void {
		if (checked) {
			checkedIds = new Set(BOOK_FIXTURES.map((b) => b.id));
		} else {
			checkedIds = new Set();
		}
	}

	function toggleOp(index: number): void {
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
					onclick={() => setSurface('rail')}
				>
					Rail
				</button>
				<button
					type="button"
					class:on={surface === 'popover'}
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
					class:on={selectionDemo === 'single'}
					onclick={() => setSelectionDemo('single')}
				>
					Single file
				</button>
				<button
					type="button"
					class:on={selectionDemo === 'multi'}
					onclick={() => setSelectionDemo('multi')}
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
					class:on={!opsOpen}
					onclick={() => {
						opsOpen = false;
					}}
				>
					Collapsed
				</button>
				<button
					type="button"
					class:on={opsOpen}
					onclick={() => {
						opsOpen = true;
					}}
				>
					Pinned open
				</button>
			</div>
		</div>
	</div>

	<div class="proto-window" class:compact={density === 'compact'}>
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
					onclick={() => setDensity('comfortable')}
				>
					Comfortable
				</button>
				<button
					type="button"
					class:on={density === 'compact'}
					onclick={() => setDensity('compact')}
				>
					Compact
				</button>
			</div>
		</div>

		<div class="proto-toolbar" class:multi={multiMode}>
			<button type="button" class="btn-pill btn-pill-secondary">＋ Import</button>
			<button type="button" class="btn-pill btn-pill-secondary">☁ Audible</button>
			<span class="proto-badge proto-badge-info proto-single-hint">merge — 3 files → one M4B</span>
			<div class="proto-sel-actions">
				<span class="proto-badge proto-badge-info">{selectedCount} selected</span>
				<button type="button" class="btn-pill btn-pill-secondary proto-btn-sm">
					Find metadata ({selectedCount})
				</button>
				<button type="button" class="btn-pill btn-pill-secondary proto-btn-sm">
					Edit shared fields
				</button>
				<button type="button" class="btn-pill btn-pill-secondary proto-btn-sm">Remove</button>
			</div>
			<span class="proto-toolbar-spacer"></span>
			<button type="button" class="btn-pill btn-pill-secondary">FDK HE-AAC · VBR 3 ▾</button>
			<button type="button" class="btn-pill btn-pill-secondary">ABS naming ▾</button>
			<button type="button" class="btn-pill btn-pill-primary">Process</button>
		</div>

		<div class="proto-main" class:no-rail={!railVisible}>
			<div class="proto-left">
				<div class="proto-table-wrap">
					<table class="proto-tbl">
						<thead>
							<tr>
								<th class="proto-ck">
									<input
										type="checkbox"
										checked={checkedIds.size === BOOK_FIXTURES.length}
										onchange={(e) => toggleSelectAll(e.currentTarget.checked)}
									/>
								</th>
								<th style="width:34px"></th>
								<th>Book</th>
								<th class="proto-comfy-only">Author</th>
								<th style="text-align:right">Duration</th>
								<th style="text-align:right" class="proto-comfy-only">Size</th>
								<th class="proto-comfy-only">Codec</th>
								<th>Status</th>
							</tr>
						</thead>
						<tbody>
							{#each BOOK_FIXTURES as book (book.id)}
								{@const badge = STATUS_BADGE[book.status]}
								<tr
									class:sel={isRowSelected(book.id)}
									onclick={(e) => selectRow(book, e)}
								>
									<td class="proto-ck">
										<input
											type="checkbox"
											checked={isRowSelected(book.id)}
											onchange={(e) => toggleRowCheck(book, e.currentTarget.checked)}
											onclick={(e) => e.stopPropagation()}
										/>
									</td>
									<td class="proto-cover-cell">
										<div
											class="app-cover-thumb proto-row-cover"
											style={`--cover-thumb-size: ${density === 'compact' ? '1.125rem' : '1.625rem'}; background: ${book.gradient}`}
										></div>
									</td>
									<td>{book.title}</td>
									<td class="proto-comfy-only">{book.author}</td>
									<td class="proto-n">{book.duration}</td>
									<td class="proto-n proto-comfy-only">{book.size}</td>
									<td class="proto-comfy-only">{book.codec}</td>
									<td>
										<span class="proto-badge proto-badge-{badge.class}">{badge.label}</span>
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>

				<div class="proto-ops" class:open={opsOpen}>
					<div
						class="proto-ops-bar"
						role="button"
						tabindex="0"
						onclick={() => {
							if (!opsPinned) opsOpen = !opsOpen;
						}}
						onkeydown={(e) => {
							if (e.key === 'Enter' || e.key === ' ') {
								e.preventDefault();
								if (!opsPinned) opsOpen = !opsOpen;
							}
						}}
					>
						<button
							type="button"
							class="btn-pill btn-pill-secondary proto-btn-sm"
							onclick={(e) => e.stopPropagation()}
						>
							▶ 30s
						</button>
						<div class="proto-transport-track">
							<div class="app-progress-track">
								<div class="app-progress-fill" style="width: 64%"></div>
							</div>
						</div>
						<span class="proto-transport-label">
							The Way of Kings · 64% · 04:02 left
						</span>
						<span class="proto-ops-meta">
							<span class="proto-badge proto-badge-info">1 running</span>
							<span class="proto-badge proto-badge-mut">1 queued</span>
							<span class="proto-badge proto-badge-ok">2 done</span>
							<span>5 books · <span class="proto-mono">62:46:27 · 1.72 GB</span></span>
							<button
								type="button"
								class="proto-pin"
								class:on={opsPinned}
								onclick={(e) => {
									e.stopPropagation();
									opsPinned = !opsPinned;
								}}
							>
								⚲ {opsPinned ? 'pinned' : 'unpinned'}
							</button>
							<span>▾</span>
						</span>
					</div>
					<div class="proto-ops-body">
						<div class="proto-op" class:open={expandedOp === 0}>
							<div
								class="proto-op-row"
								role="button"
								tabindex="0"
								onclick={() => toggleOp(0)}
								onkeydown={(e) => {
									if (e.key === 'Enter' || e.key === ' ') {
										e.preventDefault();
										toggleOp(0);
									}
								}}
							>
								<span class="proto-badge proto-badge-info">merge</span>
								<span class="proto-op-title">The Way of Kings — 3 files → M4B</span>
								<span class="proto-mono proto-op-pct">64%</span>
								<button
									type="button"
									class="btn-pill btn-pill-secondary proto-btn-xs"
									onclick={(e) => e.stopPropagation()}
								>
									Cancel
								</button>
							</div>
							<div class="proto-op-detail">
								<div class="proto-lane">
									<span>analysis</span>
									<div class="app-progress-track proto-lane-track">
										<div
											class="app-progress-fill"
											style="width:100%; background: var(--text-success)"
										></div>
									</div>
									<span class="proto-mono">done</span>
								</div>
								<div class="proto-lane">
									<span>encode</span>
									<div class="app-progress-track proto-lane-track">
										<div class="app-progress-fill" style="width:64%"></div>
									</div>
									<span class="proto-mono">04:02</span>
								</div>
								<div class="proto-lane">
									<span>commit</span>
									<div class="app-progress-track proto-lane-track">
										<div class="app-progress-fill" style="width:0%"></div>
									</div>
									<span class="proto-mono">—</span>
								</div>
								<div class="proto-op-log">
									<b>10:22:11</b> encoding chunk 8/12 · 12.1 MB/s<br />
									<b>10:22:40</b> chapters synthesized · 75 markers
								</div>
							</div>
						</div>
						<div class="proto-op" class:open={expandedOp === 1}>
							<button type="button" class="proto-op-row" onclick={() => toggleOp(1)}>
								<span class="proto-badge proto-badge-mut">queued</span>
								<span class="proto-op-title muted-text">Emergent Strategy — batch encode</span>
								<span class="proto-mono proto-op-pct muted-text">#2</span>
							</button>
						</div>
						<div class="proto-op proto-op-done" class:open={expandedOp === 2}>
							<button type="button" class="proto-op-row" onclick={() => toggleOp(2)}>
								<span class="proto-badge proto-badge-ok">done</span>
								<span class="proto-op-title">The Martian — batch encode</span>
								<span class="proto-mono proto-op-pct muted-text">2m ago</span>
							</button>
							<div class="proto-op-detail">
								<div class="proto-op-log">
									<b>terminal</b> success · output verified · 298.4 MB
								</div>
							</div>
						</div>
					</div>
				</div>
			</div>

			{#if railVisible}
				<aside class="proto-rail">
					<div class="proto-rail-head">
						<div
							class="app-cover-thumb proto-rail-cover"
							style={`background: ${railGradient}`}
						></div>
						<div class="proto-rail-titles">
							<h2>{railTitle}</h2>
							<p class="muted-text">{railSubtitle}</p>
						</div>
					</div>
					<div class="proto-rail-tabs" role="tablist">
						{#each ['Metadata', 'Facts', 'Chapters', 'Output'] as tab}
							<button
								type="button"
								role="tab"
								class="proto-rail-tab"
								class:on={activeRailTab === tab}
								onclick={() => {
									activeRailTab = tab;
								}}
							>
								{tab}
							</button>
						{/each}
					</div>
					{#if multiMode}
						<div class="proto-rail-pane">
							<div class="proto-batch-note">
								Editing <b>{selectedCount} files</b>. Shared fields apply to every selected file
								on save; mixed values stay untouched unless you overwrite them.
							</div>
							<div class="proto-field">
								<label for="rail-author">Author</label>
								<input id="rail-author" type="text" placeholder="Mixed (3 values)" class="mixed" />
							</div>
							<div class="proto-field">
								<label for="rail-series">Series</label>
								<input id="rail-series" type="text" value="" />
							</div>
							<div class="proto-field">
								<label for="rail-genre">Genre</label>
								<input id="rail-genre" type="text" value="Science Fiction" />
							</div>
							<div class="proto-rail-actions">
								<button type="button" class="btn-pill btn-pill-primary" style="flex:1">
									Save to {selectedCount} files
								</button>
								<button type="button" class="btn-pill btn-pill-secondary">
									Find metadata ({selectedCount})
								</button>
							</div>
						</div>
					{:else}
						<div class="proto-rail-pane">
							<div class="proto-field">
								<label for="rail-title-field">Book title</label>
								<input id="rail-title-field" type="text" value={selectedBook.title} />
							</div>
							<div class="proto-field">
								<label for="rail-author-field">Author</label>
								<input id="rail-author-field" type="text" value={selectedBook.author} />
							</div>
							<div class="proto-field">
								<label for="rail-narrator">Narrator</label>
								<input
									id="rail-narrator"
									type="text"
									value="Michael Kramer, Kate Reading"
								/>
							</div>
							<div class="proto-field-grid">
								<div class="proto-field">
									<label for="rail-series-name">Series</label>
									<input id="rail-series-name" type="text" value="The Stormlight Archive" />
								</div>
								<div class="proto-field">
									<label for="rail-book-num">Book #</label>
									<input id="rail-book-num" type="text" value="1" />
								</div>
							</div>
							<div class="proto-rail-actions">
								<button type="button" class="btn-pill btn-pill-primary" style="flex:1">
									Save
								</button>
								<button type="button" class="btn-pill btn-pill-secondary">Find online</button>
							</div>
						</div>
					{/if}
				</aside>
			{/if}
		</div>

		{#if surface === 'popover' && popoverOpen}
			<div class="proto-pop open" role="dialog" aria-label="Metadata editor">
				<div class="proto-pop-head">
					<div
						class="app-cover-thumb proto-pop-cover"
						style={`background: ${selectedBook.gradient}`}
					></div>
					<span class="proto-pop-title">{railTitle}</span>
					<button
						type="button"
						class="btn-pill btn-pill-secondary proto-btn-xs"
						onclick={() => {
							popoverOpen = false;
						}}
					>
						✕
					</button>
				</div>
				{#if multiMode}
					<div class="proto-pop-body">
						<div class="proto-batch-note">
							Editing <b>{selectedCount} files</b> — shared fields only.
						</div>
						<div class="proto-field">
							<label for="pop-author">Author</label>
							<input id="pop-author" type="text" placeholder="Mixed (3 values)" class="mixed" />
						</div>
						<div class="proto-field">
							<label for="pop-series">Series</label>
							<input id="pop-series" type="text" value="" />
						</div>
						<div class="proto-pop-actions">
							<button type="button" class="btn-pill btn-pill-primary" style="flex:1">
								Save to {selectedCount} files
							</button>
							<button type="button" class="btn-pill btn-pill-secondary">
								Find metadata ({selectedCount})
							</button>
						</div>
					</div>
				{:else}
					<div class="proto-pop-body">
						<div class="proto-field">
							<label for="pop-title">Book title</label>
							<input id="pop-title" type="text" value={selectedBook.title} />
						</div>
						<div class="proto-field">
							<label for="pop-author-single">Author</label>
							<input id="pop-author-single" type="text" value={selectedBook.author} />
						</div>
						<div class="proto-field-grid">
							<div class="proto-field">
								<label for="pop-series-name">Series</label>
								<input id="pop-series-name" type="text" value="The Stormlight Archive" />
							</div>
							<div class="proto-field">
								<label for="pop-book-num">Book #</label>
								<input id="pop-book-num" type="text" value="1" />
							</div>
						</div>
						<div class="proto-pop-actions">
							<button type="button" class="btn-pill btn-pill-primary" style="flex:1">Save</button>
							<button type="button" class="btn-pill btn-pill-secondary">Find online</button>
						</div>
					</div>
				{/if}
			</div>
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
		overflow: hidden;
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
		display: none;
		align-items: center;
		gap: var(--space-2);
	}

	.proto-toolbar.multi .proto-sel-actions {
		display: flex;
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

	.proto-btn-xs {
		padding: 3px 10px !important;
		font-size: var(--text-xs) !important;
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

	.proto-badge-ok {
		background: color-mix(in srgb, var(--text-success) 14%, transparent);
		color: var(--text-success);
	}

	.proto-badge-info {
		background: color-mix(in srgb, var(--accent-primary) 16%, transparent);
		color: var(--accent-primary-hover);
	}

	.proto-badge-warn {
		background: color-mix(in srgb, #fbbf24 14%, transparent);
		color: #fbbf24;
	}

	.proto-badge-mut {
		background: var(--bg-input);
		color: var(--text-muted);
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

	.proto-table-wrap {
		flex: 1;
		overflow: auto;
	}

	.proto-tbl {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--density-text);
	}

	.proto-tbl th {
		text-align: left;
		font-size: var(--text-xs);
		font-weight: 600;
		letter-spacing: 0.05em;
		text-transform: uppercase;
		color: var(--text-placeholder);
		padding: var(--space-2) var(--space-3);
		border-bottom: 1px solid var(--border-primary);
		position: sticky;
		top: 0;
		background: var(--bg-main);
		z-index: 1;
	}

	.proto-tbl td {
		padding: 0 var(--space-3);
		height: var(--density-row-h);
		border-bottom: 1px solid var(--border-primary);
		color: var(--text-secondary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.proto-tbl tbody tr {
		cursor: pointer;
	}

	.proto-tbl tbody tr:hover {
		background: var(--bg-panel);
	}

	.proto-tbl tbody tr.sel {
		background: color-mix(in srgb, var(--accent-primary) 12%, transparent);
		box-shadow: inset 2px 0 0 var(--accent-primary);
	}

	.proto-n {
		font-family: var(--font-mono);
		text-align: right;
		color: var(--text-muted);
		font-size: var(--text-sm);
	}

	.proto-ck {
		width: 1.875rem;
	}

	.proto-ck input {
		margin-top: 0;
		accent-color: var(--accent-primary);
	}

	.proto-comfy-only {
		display: table-cell;
	}

	.proto-window.compact .proto-comfy-only {
		display: none;
	}

	.proto-row-cover {
		border: none;
	}

	.proto-ops {
		border-top: 1px solid var(--border-primary);
		background: var(--bg-panel);
		flex-shrink: 0;
	}

	.proto-ops-bar {
		display: flex;
		align-items: center;
		gap: var(--space-3);
		width: 100%;
		padding: 9px var(--space-4);
		border: none;
		background: transparent;
		color: inherit;
		cursor: pointer;
		text-align: left;
	}

	.proto-transport-track {
		flex: 1;
		max-width: 23.75rem;
	}

	.proto-transport-label {
		font-size: var(--text-sm);
		color: var(--text-secondary);
		white-space: nowrap;
	}

	.proto-ops-meta {
		margin-left: auto;
		display: flex;
		align-items: center;
		gap: var(--space-2);
		font-size: var(--text-sm);
		color: var(--text-muted);
		flex-wrap: wrap;
		justify-content: flex-end;
	}

	.proto-mono {
		font-family: var(--font-mono);
	}

	.proto-pin {
		margin-top: 0;
		background: none;
		border: none;
		color: var(--text-placeholder);
		font-size: var(--text-sm);
		padding: 4px;
		cursor: pointer;
	}

	.proto-pin.on {
		color: var(--accent-primary-hover);
	}

	.proto-ops-body {
		display: none;
		max-height: 13.75rem;
		overflow: auto;
		padding: 0 var(--space-4) var(--space-3);
		flex-direction: column;
		gap: var(--space-2);
	}

	.proto-ops.open .proto-ops-body {
		display: flex;
	}

	.proto-op {
		border: 1px solid var(--border-primary);
		border-radius: var(--radius-md);
	}

	.proto-op-done {
		opacity: 0.6;
	}

	.proto-op-row {
		display: flex;
		align-items: center;
		gap: var(--space-2);
		width: 100%;
		padding: var(--space-2) var(--space-3);
		border: none;
		background: transparent;
		font-size: var(--text-sm);
		color: inherit;
		cursor: pointer;
		text-align: left;
	}

	.proto-op-row:hover {
		background: var(--bg-input);
	}

	.proto-op-title {
		flex: 1;
		font-weight: 500;
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.proto-op-pct {
		color: var(--text-muted);
	}

	.proto-op-detail {
		display: none;
		padding: 2px var(--space-3) var(--space-2);
	}

	.proto-op.open .proto-op-detail {
		display: block;
	}

	.proto-lane {
		display: flex;
		gap: var(--space-2);
		align-items: center;
		font-size: var(--text-xs);
		color: var(--text-muted);
		padding: 4px 0;
	}

	.proto-lane span:first-child {
		width: 4.375rem;
		flex-shrink: 0;
	}

	.proto-lane-track {
		flex: 1;
	}

	.proto-op-log {
		margin-top: var(--space-2);
		background: color-mix(in srgb, var(--bg-main) 80%, #000);
		border-radius: var(--radius-sm);
		padding: var(--space-2) var(--space-2);
		font-family: var(--font-mono);
		font-size: var(--text-xs);
		line-height: 1.7;
		color: var(--text-muted);
	}

	.proto-op-log b {
		color: var(--text-secondary);
		font-weight: 500;
	}

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

	.proto-rail-tabs {
		display: flex;
		padding: var(--space-2) var(--space-3) 0;
		border-bottom: 1px solid var(--border-primary);
	}

	.proto-rail-tab {
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

	.proto-rail-tab.on {
		color: var(--text-primary);
		border-bottom-color: var(--accent-primary);
	}

	.proto-rail-pane,
	.proto-pop-body {
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

	.proto-rail-actions,
	.proto-pop-actions {
		display: flex;
		gap: var(--space-2);
		margin-top: 2px;
	}

	.proto-pop {
		position: absolute;
		top: 10.625rem;
		left: 26.25rem;
		width: 20.625rem;
		background: var(--bg-panel);
		border: 1px solid var(--border-secondary);
		border-radius: var(--radius-lg);
		box-shadow: var(--shadow-md);
		z-index: 20;
		display: none;
	}

	.proto-pop.open {
		display: block;
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
</style>
