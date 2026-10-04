import { For, Show } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { useAppRuntime } from '../../app/runtime';
import { pathBasename } from '../../lib/path/basename';
import { Button, Dialog } from '../foundation';
import type { CollisionPolicy, OutputCollisionKind, PlannedOutput } from '../../types/audio';
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

function isRepeatPress(event: MouseEvent): boolean {
	return event.detail > 1;
}

export function CollisionDialogView(): JSX.Element {
	const view = useAppRuntime().output.collision;
	return (
		<>
			<Show when={view().reviewId} keyed>
				{(reviewId) => <CollisionQuestion reviewId={reviewId} />}
			</Show>
			<RestartDialogView />
		</>
	);
}

function CollisionQuestion(props: { readonly reviewId: number }): JSX.Element {
	const output = useAppRuntime().output;
	const view = output.collision;
	const cancel = () => output.cancelCollisionReview(props.reviewId);
	const choose = (policy: CollisionPolicy) => (event: MouseEvent) => {
		if (!isRepeatPress(event)) output.chooseCollisionPolicy(props.reviewId, policy);
	};

	return (
		<Dialog
			id="collision-dialog-modal"
			open={true}
			onClose={cancel}
			labelledBy="collision-dialog-title"
			testId="collision-dialog-modal"
		>
			<Dialog.Header>
				<h3 id="collision-dialog-title">{view().title}</h3>
				<Button id="collision-dialog-close" data-testid="collision-dialog-close" onClick={cancel}>
					Cancel
				</Button>
			</Dialog.Header>

			<Dialog.Body>
				<p id="collision-dialog-body" class="muted-text">
					{view().body}
				</p>

				<div id="collision-dialog-results" class="app-modal-results">
					<For each={view().outputs}>
						{(outputItem) => (
							<div
								class="app-modal-result collision-dialog-result"
								data-testid="collision-dialog-item"
							>
								<div class="collision-dialog-paths">
									<div class="collision-dialog-filename" title={outputItem.resolvedPath}>
										{pathBasename(outputItem.resolvedPath, { fallback: 'path' })}
									</div>
									<div class="collision-dialog-parent-path" title={outputItem.resolvedPath}>
										{parentPath(outputItem.resolvedPath)}
									</div>
								</div>
								<Show when={outputItem.collision && outputItem.collision.kind !== 'existing_file'}>
									<div
										class="collision-dialog-summary"
										title={outputItem.collision?.detail ?? undefined}
									>
										{formatOutputKind(outputItem.kind)} •{' '}
										{formatKind(outputItem.collision?.kind ?? 'existing_file')}
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
							onClick={choose('replace_existing')}
						>
							Overwrite Existing
						</Button>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<Button
							id="collision-dialog-skip"
							data-testid="collision-dialog-skip"
							onClick={choose('skip_existing')}
						>
							Skip Existing
						</Button>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<Button
							id="collision-dialog-rename"
							data-testid="collision-dialog-rename"
							onClick={choose('rename_new')}
						>
							Keep Existing
						</Button>
					</div>
					<div class="app-modal-field app-modal-field-button">
						<Button
							id="collision-dialog-cancel"
							data-testid="collision-dialog-cancel"
							onClick={cancel}
						>
							Cancel
						</Button>
					</div>
				</div>
			</Dialog.Body>
		</Dialog>
	);
}

function RestartDialogView(): JSX.Element {
	const processing = useAppRuntime().processing;
	return (
		<Show when={processing.restartPrompt()}>
			{(offer) => (
				<Dialog
					open={true}
					onClose={() => void processing.keepLocation(offer())}
					labelledBy="restart-dialog-title"
					testId="restart-dialog-modal"
				>
					<Dialog.Header>
						<h3 id="restart-dialog-title">Restart this export?</h3>
					</Dialog.Header>
					<Dialog.Body>
						<p>This Save changes where the audiobook goes.</p>
						<p class="restart-dialog-paths">
							<strong>From:</strong> {offer().from}
							<br />
							<strong>To:</strong> {offer().to}
						</p>
						<p>
							Restart it at the new location? Its unfinished output and any empty folders made for
							it are removed. Keep Location lets the export finish where it is.
						</p>
						<div class="restart-dialog-controls">
							<Button
								tone="primary"
								onClick={(event) => {
									if (!isRepeatPress(event)) void processing.restart(offer());
								}}
							>
								Restart
							</Button>
							<Button onClick={() => void processing.keepLocation(offer())}>Keep Location</Button>
						</div>
					</Dialog.Body>
				</Dialog>
			)}
		</Show>
	);
}
