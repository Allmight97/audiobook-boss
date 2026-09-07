import { Show } from 'solid-js';
import type { JSX } from '@solidjs/web';
import { useAppRuntime } from '../../app/runtime';
import { Button } from '../foundation';
import './appSettingsDialog.css';

export function SettingsPersistenceNotice(): JSX.Element {
	const settings = useAppRuntime().settings;
	const durability = settings.durability;
	return (
		<Show when={durability().message || settings.concurrency().errorMessage}>
			<div class="settings-persistence-notice" role="status" aria-live="polite">
				<Show when={durability().message}>
					<div class="settings-persistence-copy">
						<strong>Settings haven't been saved</strong>
						<p>Your current choices still apply for this session. {durability().message}</p>
					</div>
					<Button
						disabled={durability().state === 'saving'}
						onClick={() => void settings.retryPersistence()}
					>
						{durability().state === 'saving' ? 'Saving…' : 'Retry save'}
					</Button>
				</Show>
				<Show when={settings.concurrency().errorMessage}>
					<p class="settings-concurrency-error">
						Couldn't change the job limit. {settings.concurrency().errorMessage}
					</p>
				</Show>
			</div>
		</Show>
	);
}
