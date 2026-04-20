<script lang="ts">
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
		const segments = path.split(/[\\/]/).filter((segment) => segment.length > 0);
		return segments[segments.length - 1] ?? path;
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
	class="metadata-lookup-modal"
	class:open={collisionDialogState.isOpen}
	data-testid="collision-dialog-modal"
	aria-hidden={!collisionDialogState.isOpen}
	on:click={handleBackdropClick}
>
	<div class="metadata-lookup-dialog" role="dialog" aria-modal="true" aria-labelledby="collision-dialog-title">
		<div class="metadata-lookup-header">
			<h3 id="collision-dialog-title">{collisionDialogState.title}</h3>
			<button
				id="collision-dialog-close"
				class="btn-pill btn-pill-secondary"
				data-testid="collision-dialog-close"
				type="button"
				on:click={cancelCollisionDialog}
			>
				Cancel
			</button>
		</div>

		<div class="metadata-lookup-body">
			<p id="collision-dialog-body" class="text-xs muted-text">{collisionDialogState.body}</p>

			<div id="collision-dialog-results" class="metadata-lookup-results">
				{#each collisionDialogState.outputs as output}
					<div class="metadata-lookup-result collision-dialog-result" data-testid="collision-dialog-item">
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

			<div class="metadata-lookup-controls collision-dialog-controls" style="margin-top: 1rem;">
				<div class="metadata-lookup-field metadata-lookup-field-button">
					<button
						id="collision-dialog-replace"
						class="btn-pill btn-pill-primary"
						data-testid="collision-dialog-replace"
						type="button"
						on:click={() => chooseCollisionPolicy('replace_existing')}
					>
						Overwrite Existing
					</button>
				</div>
				<div class="metadata-lookup-field metadata-lookup-field-button">
					<button
						id="collision-dialog-skip"
						class="btn-pill btn-pill-secondary"
						data-testid="collision-dialog-skip"
						type="button"
						on:click={() => chooseCollisionPolicy('skip_existing')}
					>
						Skip Existing
					</button>
				</div>
				<div class="metadata-lookup-field metadata-lookup-field-button">
					<button
						id="collision-dialog-rename"
						class="btn-pill btn-pill-secondary"
						data-testid="collision-dialog-rename"
						type="button"
						on:click={() => chooseCollisionPolicy('rename_new')}
					>
						Keep Existing
					</button>
				</div>
				<div class="metadata-lookup-field metadata-lookup-field-button">
					<button
						id="collision-dialog-cancel"
						class="btn-pill btn-pill-secondary"
						data-testid="collision-dialog-cancel"
						type="button"
						on:click={cancelCollisionDialog}
					>
						Cancel
					</button>
				</div>
			</div>
		</div>
	</div>
</div>
