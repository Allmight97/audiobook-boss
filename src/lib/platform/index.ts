export const QUIT_SHORTCUT = '⌘Q';

export function runsOnMac(): boolean {
	return typeof navigator !== 'undefined' && navigator.userAgent.includes('Mac');
}

export function fileManagerLabel(): string {
	return runsOnMac() ? 'Show in Finder' : 'Show in Folder';
}

export function multiSelectClickLabel(): string {
	return runsOnMac() ? '⌘-click' : 'Ctrl-click';
}
