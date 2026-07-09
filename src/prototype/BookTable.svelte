<script lang="ts">
	import { BOOK_FIXTURES, STATUS_BADGE, type BookFixture } from './fixtures';
	import type { PrototypeSelection } from './selection.svelte';

	type Density = 'comfortable' | 'compact';

	type Props = {
		selection: PrototypeSelection;
		density: Density;
		onActivateBook: (book: BookFixture, rowEl: HTMLTableRowElement) => void;
	};

	let { selection, density, onActivateBook }: Props = $props();

	let selectAllEl = $state<HTMLInputElement | null>(null);

	$effect(() => {
		if (selectAllEl) {
			selectAllEl.indeterminate = selection.isIndeterminate();
		}
	});

	function toggleSelectAll(checked: boolean): void {
		selection.apply(checked ? { type: 'selectAll' } : { type: 'clear' });
	}

	function toggleRow(book: BookFixture, checked: boolean): void {
		const wasSelected = selection.isSelected(book.id);
		selection.apply({ type: 'toggle', id: book.id });
		if (checked && !wasSelected) {
			selection.setActiveId(book.id);
		}
	}

	function activateRow(book: BookFixture, rowEl: HTMLTableRowElement): void {
		selection.apply({ type: 'selectOnly', id: book.id });
		onActivateBook(book, rowEl);
	}
</script>

<div class="proto-table-wrap">
	<table class="proto-tbl" data-testid="book-table">
		<thead>
			<tr>
				<th class="proto-ck">
					<input
						bind:this={selectAllEl}
						type="checkbox"
						aria-label="Select all books"
						checked={selection.isAllSelected()}
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
				{@const selected = selection.isSelected(book.id)}
				{@const active = selection.activeId === book.id}
				<tr
					class:sel={selected}
					class:active={active}
					data-testid={`book-row-${book.id}`}
				>
					<td class="proto-ck">
						<input
							type="checkbox"
							aria-label={`Select ${book.title}`}
							checked={selected}
							onchange={(e) => toggleRow(book, e.currentTarget.checked)}
						/>
					</td>
					<td class="proto-cover-cell">
						<div
							class="app-cover-thumb proto-row-cover"
							style={`--cover-thumb-size: ${density === 'compact' ? '1.125rem' : '1.625rem'}; background: ${book.gradient}`}
						></div>
					</td>
					<td>
						<button
							type="button"
							class="proto-book-active"
							aria-pressed={active}
							aria-label={`Edit metadata for ${book.title}`}
							onclick={(e) => activateRow(book, e.currentTarget.closest('tr')!)}
						>
							{book.title}
						</button>
					</td>
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

<style>
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

	.proto-tbl tbody tr:hover {
		background: var(--bg-panel);
	}

	.proto-tbl tbody tr.sel {
		background: color-mix(in srgb, var(--accent-primary) 12%, transparent);
		box-shadow: inset 2px 0 0 var(--accent-primary);
	}

	.proto-tbl tbody tr.active .proto-book-active {
		font-weight: 600;
		color: var(--text-primary);
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

	:global(.proto-window.compact) .proto-comfy-only {
		display: none;
	}

	.proto-row-cover {
		border: none;
	}

	.proto-book-active {
		margin-top: 0;
		padding: 0;
		border: none;
		background: transparent;
		color: inherit;
		font: inherit;
		text-align: left;
		cursor: pointer;
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.proto-book-active:hover {
		color: var(--text-primary);
		text-decoration: underline;
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
</style>
