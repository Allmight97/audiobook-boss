import * as feedback from './feedback';

export function updateStatusPanelConcurrencyStatus(message: string): void {
	feedback.updateConcurrencyStatus(message);
}
