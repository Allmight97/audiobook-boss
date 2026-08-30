import { createSignal, For, onCleanup, onMount, Show, type JSX } from 'solid-js';
import {
	cancelCollisionDialog,
	chooseCollisionPolicy,
	getCollisionView,
	subscribeCollisionView,
} from '../../app/outputPlan';
import { pathBasename } from '../../lib/path/basename';
import { Button, Dialog } from '../foundation';
import type { OutputCollisionKind, PlannedOutput } from '../../types/audio';
import './collisionDialog.css';

function formatKind(kind: OutputCollisionKind): string {
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

function formatOutputKind(kind: PlannedOutput['kind']): string {
	return kind === 'preview' ? 'Preview' : 'Final';
}

function parentPath(path: string): string {
	const normalized = path.replace(/[\\/]+$/, '');
	const lastSeparator = Math.max(normalized.lastIndexOf('/'), normalized.lastIndexOf('\\'));
	if (lastSeparator <= 0) {
		return normalized;
	}
	return normalized.slice(0, lastSeparator);
}

export function CollisionDialogView(): JSX.Element {
	const [revision, setRevision] = createSignal(0);
	const view = () => {
		revision();
		return getCollisionView();
	};

	onMount(() => {
		onCleanup(subscribeCollisionView(() => setRevision((value) => value + 1)));
	});

	return (
		<Dialog
			id="collision-dialog-modal"
			open={view().isOpen}
			onClose={cancelCollisionDialog}
			labelledBy="collision-dialog-title"
			testId="collision-dialog-modal"
		>
			<Dialog.Header>
				<h3 id="collision-dialog-title">{view().title}</h3>
				<Button
					id="collision-dialog-close"
					data-testid="collision-dialog-close"
					onClick={cancelCollisionDialog}
				>
					Cancel
				</Button>
			</Dialog.Header>

			<Dialog.Body>
				<p id="collision-dialog-body" class="muted-text">
					{view().body}
				</p>

				<div id="collision-dialog-results" class="app-modal-results">
					<For each={view().outputs}>
						{(output) => (
							<div
								class="app-modal-result collision-dialog-result"
								data-testid="collision-dialog-item"
							>
								<div class="collision-dialog-paths">
									<div class="collision-dialog-filename" title={output.resolvedPath}>
										{pathBasename(output.resolvedPath, { fallback: 'path' })}
									</div>
									<div class="collision-dialog-parent-path" title={output.resolvedPath}>
										{parentPath(output.resolvedPath)}
									</div>
								</div>
								<Show when={output.collision && output.collision.kind !== 'existing_file'}>
									<div
										class="collision-dialog-summary"
										title={output.collision?.detail ?? undefined}
									>
										{formatOutputKind(output.kind)} •{' '}
										{formatKind(output.collision?.kind ?? 'existing_file')}
									</div>
								</Show>
							</div>
						)}
					</For>
				</div>

				<div class="app-modal-controls collision-dialog-controls">
					<div class="app-modal-field app-modal-field-button">
						<Button
							id="collision-dialog-replace"
							tone="primary"
							data-testid="collision-dialog-replace"
							onClick={() => chooseCollisionPolicy('replace_existing')}
						>
							Overwrite Existing
						</Button>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<Button
							id="collision-dialog-skip"
							data-testid="collision-dialog-skip"
							onClick={() => chooseCollisionPolicy('skip_existing')}
						>
							Skip Existing
						</Button>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<Button
							id="collision-dialog-rename"
							data-testid="collision-dialog-rename"
							onClick={() => chooseCollisionPolicy('rename_new')}
						>
							Keep Existing
						</Button>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<Button
							id="collision-dialog-cancel"
							data-testid="collision-dialog-cancel"
							onClick={cancelCollisionDialog}
						>
							Cancel
						</Button>
					</div>
				</div>
			</Dialog.Body>
		</Dialog>
	);
}
