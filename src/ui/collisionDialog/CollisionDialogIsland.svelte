<script lang="ts">
	import { pathBasename } from '../../lib/path/basename';
	import {
		cancelCollisionDialog,
		chooseCollisionPolicy,
		collisionDialogState,
	} from './state.svelte';

	function handleBackdropClick(event: MouseEvent): void {
		if (event.target === event.currentTarget) {
			cancelCollisionDialog();
		}
	}

	function formatKind(kind: NonNullable<(typeof collisionDialogState.outputs)[number]['collision']>['kind']): string {
		switch (kind) {
			case 'existing_file':
				return 'Existing file';
			case 'batch_duplicate':
				return 'Batch duplicate';
			case 'source_destination_overlap':
				return 'Source overlap';
			case 'canonical_path_overlap':
				return 'Canonical overlap';
			case 'case_insensitive_match':
				return 'Case-insensitive match';
			default:
				return kind;
		}
	}

	function formatOutputKind(kind: (typeof collisionDialogState.outputs)[number]['kind']): string {
		return kind === 'preview' ? 'Preview' : 'Final';
	}

	function getBasename(path: string): string {
		return pathBasename(path, { trimTrailingSeparators: true });
	}

	function getParentPath(path: string): string {
		const normalized = path.replace(/[\\/]+$/, '');
		const lastSeparator = Math.max(normalized.lastIndexOf('/'), normalized.lastIndexOf('\\'));
		if (lastSeparator <= 0) {
			return normalized;
		}
		return normalized.slice(0, lastSeparator);
	}

</script>

<div
	id="collision-dialog-modal"
	class="app-modal-backdrop"
	class:open={collisionDialogState.isOpen}
	data-testid="collision-dialog-modal"
	aria-hidden={!collisionDialogState.isOpen}
	onclick={handleBackdropClick}
>
	<div class="app-modal-dialog" role="dialog" aria-modal="true" aria-labelledby="collision-dialog-title">
		<div class="app-modal-header">
			<h3 id="collision-dialog-title">{collisionDialogState.title}</h3>
			<button
				id="collision-dialog-close"
				class="btn-pill btn-pill-secondary"
				data-testid="collision-dialog-close"
				type="button"
				onclick={cancelCollisionDialog}
			>
				Cancel
			</button>
		</div>

		<div class="app-modal-body">
			<p id="collision-dialog-body" class="text-xs muted-text">{collisionDialogState.body}</p>

			<div id="collision-dialog-results" class="app-modal-results">
				{#each collisionDialogState.outputs as output}
					<div class="app-modal-result collision-dialog-result" data-testid="collision-dialog-item">
						<div class="collision-dialog-paths">
							<div
								class="collision-dialog-filename"
								title={output.resolvedPath}
							>
								{getBasename(output.resolvedPath)}
							</div>
							<div
								class="collision-dialog-parent-path"
								title={output.resolvedPath}
							>
								{getParentPath(output.resolvedPath)}
							</div>
						</div>
						{#if output.collision && output.collision.kind !== 'existing_file'}
							<div class="collision-dialog-summary" title={output.collision.detail ?? undefined}>
								{formatOutputKind(output.kind)} • {formatKind(output.collision.kind)}
							</div>
						{/if}
					</div>
				{/each}
			</div>

			<div class="app-modal-controls collision-dialog-controls">
				<div class="app-modal-field app-modal-field-button">
					<button
						id="collision-dialog-replace"
						class="btn-pill btn-pill-primary"
						data-testid="collision-dialog-replace"
						type="button"
						onclick={() => chooseCollisionPolicy('replace_existing')}
					>
						Overwrite Existing
					</button>
				</div>
				<div class="app-modal-field app-modal-field-button">
					<button
						id="collision-dialog-skip"
						class="btn-pill btn-pill-secondary"
						data-testid="collision-dialog-skip"
						type="button"
						onclick={() => chooseCollisionPolicy('skip_existing')}
					>
						Skip Existing
					</button>
				</div>
				<div class="app-modal-field app-modal-field-button">
					<button
						id="collision-dialog-rename"
						class="btn-pill btn-pill-secondary"
						data-testid="collision-dialog-rename"
						type="button"
						onclick={() => chooseCollisionPolicy('rename_new')}
					>
						Keep Existing
					</button>
				</div>
				<div class="app-modal-field app-modal-field-button">
					<button
						id="collision-dialog-cancel"
						class="btn-pill btn-pill-secondary"
						data-testid="collision-dialog-cancel"
						type="button"
						onclick={cancelCollisionDialog}
					>
						Cancel
					</button>
				</div>
			</div>
		</div>
	</div>
</div>

<style>
	.collision-dialog-result {
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
	}

	.collision-dialog-controls {
		margin-top: 1rem;
		grid-template-columns: repeat(4, auto);
		justify-content: flex-start;
	}

	.collision-dialog-paths {
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
	}

	.collision-dialog-filename {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 0.95rem;
		font-weight: 600;
		color: var(--text-primary);
	}

	.collision-dialog-parent-path {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-family: var(--font-mono);
		font-size: 0.76rem;
		color: var(--text-secondary);
	}

	.collision-dialog-summary {
		max-width: 16rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		text-align: right;
		font-size: 0.76rem;
		color: var(--text-secondary);
	}

	@media (max-width: 720px) {
		.collision-dialog-controls,
		.collision-dialog-result {
			grid-template-columns: minmax(0, 1fr);
		}

		.collision-dialog-summary {
			min-width: 0;
			max-width: none;
			text-align: left;
		}
	}
</style>
