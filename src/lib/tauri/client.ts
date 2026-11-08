import { listen as tauriListen } from '@tauri-apps/api/event';
import { open as tauriOpen, type OpenDialogOptions } from '@tauri-apps/plugin-dialog';
import { openUrl as tauriOpenUrl, revealItemInDir } from '@tauri-apps/plugin-opener';

import { events as generatedEvents } from '../generated/tauri';
import { commands } from './commands';

type DialogOptions = Omit<OpenDialogOptions, 'multiple' | 'directory'>;

function openFile(options?: DialogOptions): Promise<string | null> {
	return tauriOpen({ ...options, multiple: false, directory: false } as const);
}

function openFiles(options?: DialogOptions): Promise<string[] | null> {
	return tauriOpen({ ...options, multiple: true, directory: false } as const);
}

function openDirectory(options?: DialogOptions): Promise<string | null> {
	return tauriOpen({ ...options, multiple: false, directory: true } as const);
}

export const tauriClient = {
	commands,
	events: generatedEvents,
	listen: tauriListen,
	openFile,
	openFiles,
	openDirectory,
	revealPath: (path: string): Promise<void> => revealItemInDir(path),
	openUrl: (url: string | URL): Promise<void> => tauriOpenUrl(url),
};
