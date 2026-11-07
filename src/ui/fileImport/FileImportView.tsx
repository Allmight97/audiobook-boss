import { createSignal, onSettled, Show } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { nativeDropTargetAtPoint } from '../../app/inputSession';
import { useAppRuntime } from '../../app/runtime';
import type { AcquisitionLane } from '../../lib/generated/tauri';
import { logFrontendFailure } from '../../lib/frontendLogBridge';
import { isFileDropEvent } from '../../lib/tauri/capabilities/input';
import { createSubscriptionGroup } from '../../lib/tauri/subscriptionGroup';
import { Button, SplitButton } from '../foundation';
import { FileListView } from '../fileList';
import { RemoteSourceAcquireView } from '../remoteSource';
import './fileImport.css';

export function FileImportView(): JSX.Element {
	const runtime = useAppRuntime();
	const view = runtime.input.view;
	const capability = runtime.input.capability;
	const importIntent = runtime.input.importIntent;
	const remoteSource = runtime.remoteSource;
	const defaultAcquisitionLane = runtime.settings.defaultAcquisitionLane;
	const applyCoverArtDrop = runtime.metadata.applyCoverArtDrop;
	const setDragOver = runtime.input.setDragOver;
	const [dropUnavailable, setDropUnavailable] = createSignal(false);
	let fileManagementContainer: HTMLElement | null = null;

	function openAcquire(lane: AcquisitionLane): void {
		void remoteSource.open({ lane });
	}

	onSettled(() => {
		const subscriptions = createSubscriptionGroup();
		const subscribe = (registration: Promise<() => void>): void => {
			subscriptions.add(registration).catch((error: unknown) => {
				logFrontendFailure('fileImport.dragDropListener', error);
				setDropUnavailable(true);
			});
		};
		subscribe(
			capability().listenDragEnter(() => {
				setDragOver(true);
			}),
		);
		subscribe(
			capability().listenDragLeave(() => {
				setDragOver(false);
			}),
		);
		subscribe(
			capability().listenDragDrop((payload) => {
				setDragOver(false);
				if (!isFileDropEvent(payload)) {
					return;
				}
				const coverArea = document.getElementById('cover-art-area');
				const target = nativeDropTargetAtPoint(
					payload.position,
					coverArea,
					fileManagementContainer,
				);
				if (target === 'cover') {
					void applyCoverArtDrop([...payload.paths]);
				} else if (target === 'files') {
					void importIntent({ type: 'importPaths', paths: [...payload.paths] });
				}
			}),
		);
		return () => subscriptions.dispose();
	});

	return (
		<>
			<div class="file-import-actions">
				<Button id="add-folder-btn" onClick={() => importIntent({ type: 'pickFolder' })}>
					Add Folder
				</Button>
				<SplitButton
					testId="import-split-button"
					mainId="acquire-audiobooks-btn"
					caretId="import-split-caret"
					dropdownId="import-split-dropdown"
					mainLabel="Import"
					onMainClick={() => openAcquire(defaultAcquisitionLane())}
				>
					{({ close }) => (
						<>
							<SplitButton.Option
								data-testid="import-lane-audible"
								onClick={() => {
									close();
									openAcquire('audible');
								}}
							>
								Audible
							</SplitButton.Option>
							<SplitButton.Option
								data-testid="import-lane-indexer"
								onClick={() => {
									close();
									openAcquire('indexer');
								}}
							>
								Indexer
							</SplitButton.Option>
						</>
					)}
				</SplitButton>
			</div>
			<Show when={dropUnavailable()}>
				<div class="file-import-error" role="status">
					Drag and drop is not working. Use Add Folder, or click the file list header, to import.
				</div>
			</Show>
			<Show when={view().errorMessage}>
				{(message) => (
					<div id="file-import-error" class="file-import-error">
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
