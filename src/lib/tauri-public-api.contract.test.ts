import { describe, expect, it } from 'vitest';

// oxlint-disable-next-line import/default -- Vite's ?raw import exports the file text as its default.
import generated from './generated/tauri.ts?raw';
import { TAURI_COMMAND_NAMES } from '../test/tauriCommandNames';
import { tauriClient } from './tauri/client';

const EXPECTED_EVENT_NAMES = [
	'work-operations-update',
	'session-update',
	'settings-update',
	'quit-press-again',
] as const;

const EXPECTED_TAURI_CLIENT_METHODS = [
	'commands',
	'events',
	'listen',
	'openDirectory',
	'openFile',
	'openFiles',
	'revealPath',
	'openUrl',
] as const;

describe('Tauri Runtime Boundary public API contract', () => {
	it('pins the tauriClient public method strip', () => {
		expect(Object.keys(tauriClient).toSorted()).toEqual(
			[...EXPECTED_TAURI_CLIENT_METHODS].toSorted(),
		);
	});

	it('every Rust command is one the frontend is expected to call', () => {
		const invoked = [...generated.matchAll(/__TAURI_INVOKE\("(\w+)"/g)].map((match) => match[1]);
		expect(invoked.toSorted()).toEqual([...TAURI_COMMAND_NAMES].toSorted());
	});

	it('exposes every Rust command through the unwrapped map', () => {
		const camelCase = (name: string) =>
			name.replace(/_(\w)/g, (_match, letter: string) => letter.toUpperCase());
		expect(Object.keys(tauriClient.commands).toSorted()).toEqual(
			TAURI_COMMAND_NAMES.map(camelCase).toSorted(),
		);
	});

	it('every Rust event is one the engine is expected to send', () => {
		const emitted = [...generated.matchAll(/makeEvent<\w+>\("([\w-]+)"\)/g)].map(
			(match) => match[1],
		);
		expect(emitted.toSorted()).toEqual([...EXPECTED_EVENT_NAMES].toSorted());
	});
});
