export const TAURI_COMMAND_NAMES = [
	'attach_frontend',
	'session_dispatch',
	'open_preview_output',
	'settings_dispatch',
	'cancel_work_operation',
	'log_frontend',
] as const;

export type TauriCommandName = (typeof TAURI_COMMAND_NAMES)[number];
