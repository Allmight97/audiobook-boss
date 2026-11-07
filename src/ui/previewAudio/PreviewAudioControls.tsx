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
			title={processing.previewTooltip()}
			disabled={processing.isProcessing()}
			onClick={() => void processing.preview()}
		>
			Preview Audio
		</Button>
	);
}
