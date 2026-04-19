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
					<div class="metadata-lookup-result" data-testid="collision-dialog-item">
						<div class="metadata-lookup-details">
							<div class="metadata-lookup-title">{output.resolvedPath}</div>
							<div class="metadata-lookup-meta">
								{formatOutputKind(output.kind)} • {output.collision ? formatKind(output.collision.kind) : 'No collision'}
							</div>
							{#if output.renameCandidate}
								<div class="metadata-lookup-meta">Rename target: {output.renameCandidate}</div>
							{/if}
							{#if output.collision?.detail}
								<div class="metadata-lookup-meta">{output.collision.detail}</div>
							{/if}
						</div>
					</div>
				{/each}
			</div>

			<div class="metadata-lookup-controls" style="margin-top: 1rem;">
				<div class="metadata-lookup-field metadata-lookup-field-button">
					<button
						id="collision-dialog-replace"
						class="btn-pill btn-pill-primary"
						data-testid="collision-dialog-replace"
						type="button"
						on:click={() => chooseCollisionPolicy('replace_existing')}
					>
						Overwrite Conflicts
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
						Keep Both
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
