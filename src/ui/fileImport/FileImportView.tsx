import { createEffect, onCleanup, onMount, Show, type JSX } from 'solid-js';
import {
	nativeDropLooksLikeCoverArt,
	nativeDropTargetAtPoint,
} from '../../app/inputSession/nativeIngress';
import { applyCoverArtDropAtom } from '../../app/metadataSession';
import { useAppRuntime } from '../../app/runtime';
import { useAtomSet } from '../../app/runtime/solid';
import { isFileDropEvent } from '../../types/events';
import { createSubscriptionGroup } from '../../lib/tauri/subscriptionGroup';
import { FileListView } from '../fileList/FileListView';
import { RemoteSourceAcquireView } from '../remoteSource/RemoteSourceAcquireView';

export function FileImportView(): JSX.Element {
	const runtime = useAppRuntime();
	const view = runtime.input.view;
	const capability = runtime.input.capability;
	const importIntent = runtime.input.importIntent;
	const openRemoteSourceAcquire = runtime.remoteSource.open;
	const applyCoverArtDrop = useAtomSet(() => applyCoverArtDropAtom);
	const hydrateSupportText = runtime.input.hydrateSupportText;
	const setDragOver = runtime.input.setDragOver;
	createEffect(() => {
		runtime.remoteSource.reconcileWithInput(view().files);
	});
	let fileManagementContainer: HTMLElement | null = null;
	let deferredOpenedDrain = false;

	async function drainOpenedAudioFiles(): Promise<void> {
		if (view().orderLocked) {
			deferredOpenedDrain = true;
			importIntent({ type: 'drainOpened' });
			return;
		}
		deferredOpenedDrain = false;
		importIntent({ type: 'drainOpened' });
	}

	onMount(() => {
		void hydrateSupportText();
		const subscriptions = createSubscriptionGroup();
		void subscriptions.add(
			capability().listenDragEnter(() => {
				setDragOver(true);
			}),
		);
		void subscriptions.add(
			capability().listenDragLeave(() => {
				setDragOver(false);
			}),
		);
		void subscriptions.add(
			capability().listenDragDrop((payload) => {
				setDragOver(false);
				if (!isFileDropEvent(payload)) {
					return;
				}
				const coverArea = document.getElementById('cover-art-area');
				const coverHit = nativeDropTargetAtPoint(payload.position, coverArea, null) === 'cover';
				const filesHit =
					nativeDropTargetAtPoint(payload.position, null, fileManagementContainer) === 'files';
				if (coverHit && nativeDropLooksLikeCoverArt(payload.paths)) {
					void applyCoverArtDrop([...payload.paths]);
					return;
				}
				if (filesHit) {
					importIntent({ type: 'importPaths', paths: [...payload.paths] });
				}
			}),
		);
		void subscriptions.add(
			capability().listenOpenedAudioFiles(() => {
				void drainOpenedAudioFiles();
			}),
		);
		void drainOpenedAudioFiles();
		onCleanup(() => subscriptions.dispose());
	});

	createEffect(() => {
		if (!view().orderLocked && deferredOpenedDrain) {
			void drainOpenedAudioFiles();
		}
	});

	return (
		<>
			<div class="flex items-center justify-end gap-2 mb-2">
				<button
					id="add-folder-btn"
					class="btn-pill btn-pill-secondary"
					type="button"
					onClick={() => importIntent({ type: 'pickFolder' })}
				>
					Add Folder
				</button>
				<button
					id="acquire-audiobooks-btn"
					class="btn-pill btn-pill-secondary"
					type="button"
					onClick={() => openRemoteSourceAcquire()}
				>
					Import from Library
				</button>
			</div>
			<Show when={view().errorMessage}>
				{(message) => (
					<div id="file-import-error" class="error-message mb-3" style={{ display: 'block' }}>
						{message()}
					</div>
				)}
			</Show>
			<FileListView
				onHeaderClick={() => importIntent({ type: 'pickFiles' })}
				fileManagementRef={(element) => {
					fileManagementContainer = element;
				}}
			/>
			<RemoteSourceAcquireView />
		</>
	);
}
