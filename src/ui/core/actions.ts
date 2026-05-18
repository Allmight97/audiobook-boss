import { triggerProcessFromStatusPanel } from '../statusPanel';
export { saveMetadataFromUI } from './metadataSaveWorkflow';

export function startPreviewAudio(duration: number): void {
	triggerProcessFromStatusPanel({ previewSeconds: duration });
}
