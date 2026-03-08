/**
 * StatusPanel runtime adapter.
 *
 * The singleton shell stays here so the app-level API remains stable, while the
 * processing lifecycle lives in `controller.ts` where runtime and tests can share
 * the same typed behavior surface.
 */

import * as dom from './dom';
import { StatusPanelController } from './controller';
import type { ProcessingStatus } from './state';

export class StatusPanel {
	private readonly controller: StatusPanelController;

	constructor(controller: StatusPanelController = new StatusPanelController()) {
		this.controller = controller;
	}

	public async startProcessing(options?: { previewSeconds?: number }): Promise<void> {
		return this.controller.startProcessing(options);
	}

	public async requestCancelAll(): Promise<void> {
		return this.controller.requestCancelAll();
	}

	public get isCurrentlyProcessing(): boolean {
		return this.controller.isCurrentlyProcessing;
	}

	public getCurrentStatus(): ProcessingStatus {
		return this.controller.getCurrentStatus();
	}
}

let statusPanelInstance: StatusPanel | null = null;

export function initStatusPanel(): StatusPanel {
	if (!statusPanelInstance) {
		statusPanelInstance = new StatusPanel();
	}
	return statusPanelInstance;
}

export function getStatusPanel(): StatusPanel | null {
	return statusPanelInstance;
}

export function isStatusPanelProcessing(): boolean {
	const panel = getStatusPanel();
	return Boolean(panel?.isCurrentlyProcessing);
}

export function triggerProcessFromStatusPanel(options?: { previewSeconds?: number }): void {
	const panel = getStatusPanel();
	if (!panel) return;
	void panel.startProcessing(options);
}

export function triggerCancelAllFromStatusPanel(): void {
	const panel = getStatusPanel();
	if (!panel) return;
	void panel.requestCancelAll();
}

export function pushStatusPanelTransientStatus(
	message: string,
	options?: { ttlMs?: number },
): void {
	dom.pushTransientStatusMessage(message, options?.ttlMs);
}

export function clearStatusPanelTransientStatusLock(): void {
	dom.clearTransientStatusMessageLock();
}

export { StatusPanelController } from './controller';
