import type { JSX } from '@solidjs/web';
import { useAppRuntime } from '../../app/runtime';
import { Button } from '../foundation';
import './previewAudio.css';

export function PreviewAudioControls(): JSX.Element {
	const processing = useAppRuntime().processing;

	return (
		<Button
			tone="primary"
			class="preview-audio-button"
			data-testid="preview-audio-controls"
			title="Encode a 90-second sample of each title, split across its files, with its current audio settings and metadata, so you can listen before exporting."
			disabled={processing.isProcessing()}
			onClick={() => void processing.preview()}
		>
			Preview Audio
		</Button>
	);
}
