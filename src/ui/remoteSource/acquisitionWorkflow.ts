import { tick } from 'svelte';
import { getCurrentFileList } from '../fileList';
import {
	getImportedAudioPathsBlockedMessage,
	handleImportedAudioPaths,
} from '../fileImport/handlers';
import { tauriClient } from '../../lib/tauri/client';
import {
	type AcquisitionState,
	remoteSourceProviderId,
	setAcquisitionError,
} from './acquisitionState.svelte';
import type { AcquisitionJobWithProgress } from './remoteSourceAcquireDialogHelpers';
import {
	acquisitionPollDelayMs,
	delay,
	isAcquisitionTerminal,
	statusFromAcquisitionJob,
	uniqueDiagnosticMessage,
	withClearedHandoffJob,
} from './remoteSourceAcquireDialogHelpers';
import { registerRemoteSourceSupplementalAssets } from './sessionAssets.svelte';

async function pollAcquisitionToTerminal(
	s: AcquisitionState,
	initialJob: AcquisitionJobWithProgress,
): Promise<AcquisitionJobWithProgress> {
	let currentJob = initialJob;
	while (!isAcquisitionTerminal(currentJob)) {
		await delay(acquisitionPollDelayMs);
		currentJob = (await tauriClient.getRemoteSourceAcquisitionStatus(
			currentJob.jobId,
		)) as AcquisitionJobWithProgress;
		s.activeJob = currentJob;
		s.lastJob = currentJob;
		s.statusMessage = statusFromAcquisitionJob(currentJob);
	}
	return currentJob;
}

async function finishAcquisitionJob(
	s: AcquisitionState,
	job: AcquisitionJobWithProgress,
): Promise<void> {
	const materializedPaths = job.materializedFiles.map((file) => file.path);
	if (materializedPaths.length > 0) {
		const importResult = await handleImportedAudioPaths(materializedPaths);
		if (importResult.status !== 'imported') {
			await tauriClient.purgeRemoteSourceSession(job.jobId);
			const cleanedJob = withClearedHandoffJob(job);
			s.activeJob = cleanedJob;
			s.lastJob = cleanedJob;
			s.statusMessage = `${importResult.message} Staged remote files were removed; retry acquisition after processing completes.`;
			return;
		}
		registerRemoteSourceSupplementalAssets(job, getCurrentFileList());
		s.statusMessage = `${materializedPaths.length} acquired title${materializedPaths.length === 1 ? '' : 's'} imported.`;
	} else {
		s.statusMessage =
			uniqueDiagnosticMessage(job.diagnostics) ||
			'Audible acquisition did not materialize an importable file.';
	}
}

// -- acquisition workflow controller factory --

export function createAcquisitionWorkflow(s: AcquisitionState) {
	return {
		async acquireSelected(): Promise<void> {
			if (s.selectedTitleIds.size === 0) {
				s.statusMessage = 'Select at least one Audible title.';
				return;
			}

			const blockedMessage = getImportedAudioPathsBlockedMessage();
			if (blockedMessage) {
				s.statusMessage = blockedMessage;
				return;
			}

			s.isBusy = true;
			try {
				s.activeJob = null;
				s.lastJob = null;
				s.statusMessage = 'Starting Audible acquisition.';
				const startedJob = (await tauriClient.startRemoteSourceAcquisition({
					providerId: remoteSourceProviderId,
					selections: [...s.selectedTitleIds].map((titleId) => ({
						titleId,
						includeSupplementalPdf: s.includePdfByTitleId[titleId] ?? false,
					})),
				})) as AcquisitionJobWithProgress;
				s.activeJob = startedJob;
				s.lastJob = startedJob;
				s.statusMessage = statusFromAcquisitionJob(startedJob);
				await tick();
				const terminalJob = await pollAcquisitionToTerminal(s, startedJob);
				await finishAcquisitionJob(s, terminalJob);
			} catch (cause) {
				setAcquisitionError(s, cause, 'Failed to acquire selected Audible titles.');
			} finally {
				s.isBusy = false;
			}
		},

		async cancelActiveAcquisition(): Promise<void> {
			if (!s.activeJob || isAcquisitionTerminal(s.activeJob)) return;
			try {
				const cancelledJob = (await tauriClient.cancelRemoteSourceAcquisition(
					s.activeJob.jobId,
				)) as AcquisitionJobWithProgress;
				s.activeJob = cancelledJob;
				s.lastJob = cancelledJob;
				s.statusMessage = statusFromAcquisitionJob(cancelledJob);
			} catch (cause) {
				setAcquisitionError(s, cause, 'Failed to cancel Audible acquisition.');
			}
		},
	};
}
