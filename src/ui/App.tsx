import { onSettled } from 'solid-js';
import type { JSX } from '@solidjs/web';

import { hydrateAppSettingsProduction } from '../app/appSettings';
import { useAppRuntime } from '../app/runtime';
import { AppSettingsDialogView } from './appSettings/AppSettingsDialogView';
import { CollisionDialogView } from './collisionDialog/CollisionDialogView';
import { EncoderView } from './encoderPanel';
import { OutputView } from './outputPanel/OutputView';
import { FileImportView } from './fileImport/FileImportView';
import { ConcurrencyControl } from './jobControls/ConcurrencyControl';
import { MergeModeToggle } from './jobControls/MergeModeToggle';
import { FileInspectorView } from './leftColumn/FileInspectorView';
import { MetadataLookupView } from './metadataLookup/MetadataLookupView';
import { MetadataManagerView } from './metadataManager/MetadataManagerView';
import { PreviewAudioControls } from './previewAudio/PreviewAudioControls';
import { StatusPanelView } from './statusPanel/StatusPanelView';
import { TagPreviewView } from './tagPreview/TagPreviewView';
import { WorkCenterView } from './workCenter/WorkCenterView';
import './encodingWorkbench/encodingWorkbench.css';
import './leftColumn/leftColumn.css';

export function App(): JSX.Element {
	const runtime = useAppRuntime();
	const saveMetadata = runtime.metadata.save;
	const hydrateConcurrency = runtime.settings.hydrateConcurrency;
	const openSettings = runtime.settings.openDialog;
	const applyOutputDefaults = runtime.output.applyDefaults;
	const applyEncodingDefaults = runtime.encoding.applyDefaults;
	const initializeWork = runtime.workOperations.initialize;

	onSettled(() => {
		void hydrateAppSettingsProduction().then((defaults) => {
			if (defaults) {
				applyOutputDefaults(defaults.outputDefaults);
				applyEncodingDefaults(defaults.encoderDefaults);
			}
		});
		void hydrateConcurrency();
		void initializeWork();

		function handleGlobalKeyDown(event: KeyboardEvent): void {
			if ((event.metaKey || event.ctrlKey) && event.key === 's') {
				event.preventDefault();
				void saveMetadata();
			}
			if ((event.metaKey || event.ctrlKey) && event.key === ',') {
				event.preventDefault();
				void openSettings();
			}
		}
		window.addEventListener('keydown', handleGlobalKeyDown);
		return () => window.removeEventListener('keydown', handleGlobalKeyDown);
	});

	return (
		<div class="main-container">
			<div class="panel input-panel left-column-wrapper" data-testid="left-column">
				<section
					class="left-column-panel input-workflow input-workflow-panel"
					data-testid="input-workflow-panel"
					aria-label="Input and File Order"
				>
					<div class="input-workflow-heading-row">
						<h3 class="section-title input-workflow-heading">Input and File Order</h3>
						<div class="input-workflow-controls">
							<MergeModeToggle />
							<ConcurrencyControl />
						</div>
					</div>
					<FileImportView />
				</section>
				<FileInspectorView />
			</div>

			<div class="right-column-wrapper">
				<div class="panel right-column-panel metadata-manager-panel">
					<MetadataManagerView />
				</div>
				<div class="panel right-column-panel encoding-workbench-panel">
					<div class="encoding-workbench-frame">
						<section
							class="encoding-workbench"
							aria-label="Encoding, output, and tags"
							data-testid="encoding-workbench"
						>
							<div
								class="workbench-block workbench-block-encoder"
								data-testid="encoding-workbench-encoder"
							>
								<EncoderView />
							</div>
							<div
								class="workbench-block workbench-block-output"
								data-testid="encoding-workbench-output"
							>
								<OutputView />
							</div>
							<div
								class="workbench-block workbench-block-tags"
								data-testid="encoding-workbench-tags"
							>
								<div class="workbench-block-header tags-header">
									<h3>Tags Preview</h3>
									<PreviewAudioControls variant="compact" />
								</div>
								<TagPreviewView variant="workbench" />
							</div>
						</section>
					</div>
				</div>
				<StatusPanelView />
				<WorkCenterView />
			</div>
			<MetadataLookupView />
			<AppSettingsDialogView />
			<CollisionDialogView />
		</div>
	);
}
