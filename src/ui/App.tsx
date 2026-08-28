import { onCleanup, onMount, type JSX } from 'solid-js';
import { FileImportView } from './fileImport/FileImportView';
import { MergeModeToggle } from './jobControls/MergeModeToggle';
import { FileInspectorView } from './leftColumn/FileInspectorView';
import { MetadataManagerView } from './metadataManager/MetadataManagerView';
import { TagPreviewView } from './tagPreview/TagPreviewView';
import { saveMetadataAtom } from '../app/metadataSession';
import { useAtomSet } from '../app/runtime/solid';
import './encodingWorkbench/encodingWorkbench.css';

export function App(): JSX.Element {
	const saveMetadata = useAtomSet(() => saveMetadataAtom);

	onMount(() => {
		function handleGlobalKeyDown(event: KeyboardEvent): void {
			if ((event.metaKey || event.ctrlKey) && event.key === 's') {
				event.preventDefault();
				void saveMetadata(undefined);
			}
		}
		window.addEventListener('keydown', handleGlobalKeyDown);
		onCleanup(() => window.removeEventListener('keydown', handleGlobalKeyDown));
	});

	return (
		<div class="main-container">
			<div class="panel input-panel left-column-wrapper" data-testid="left-column">
				<section
					class="left-column-panel input-workflow input-workflow-panel flex flex-col gap-2 mb-2"
					data-testid="input-workflow-panel"
					aria-label="Input and File Order"
				>
					<div class="flex items-center justify-between">
						<h3 class="section-title mb-0 whitespace-nowrap mr-2">Input and File Order</h3>
						<MergeModeToggle />
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
							class="encoding-workbench encoding-workbench-tags-only"
							aria-label="Encoding, output, and tags"
							data-testid="encoding-workbench"
						>
							<div
								class="workbench-block workbench-block-tags"
								data-testid="encoding-workbench-tags"
							>
								<div class="workbench-block-header tags-header">
									<h3>Tags Preview</h3>
								</div>
								<TagPreviewView variant="workbench" />
							</div>
						</section>
					</div>
				</div>
			</div>
		</div>
	);
}
