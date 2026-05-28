import {
	appendFileSync,
	mkdirSync,
	mkdtempSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import type { ProofEvent, ProofSummary } from './types';

export type ProofArtifactWriter = {
	artifactDir: string;
	eventsPath: string;
	logsDir: string;
	record: (event: ProofEvent) => void;
	writeSummary: (summary: ProofSummary) => { markdownPath: string; jsonPath: string };
};

function nowIso(): string {
	return new Date().toISOString();
}

function runIdPrefix(planId: string, timestamp = nowIso()): string {
	const safePlanId = planId.replace(/[^a-zA-Z0-9._-]/g, '-');
	return `${timestamp}-${safePlanId}`;
}

export function createArtifactWriter(
	repoRoot: string,
	planId: string,
	options: { timestamp?: string } = {},
): ProofArtifactWriter {
	const proofRoot = path.join(repoRoot, '.proof');
	const runsRoot = path.join(proofRoot, 'runs');
	const runPrefix = runIdPrefix(planId, options.timestamp).replace(/[:.]/g, '-');
	mkdirSync(runsRoot, { recursive: true });
	const artifactDir = mkdtempSync(path.join(runsRoot, `${runPrefix}-`));
	const logsDir = path.join(artifactDir, 'logs');
	const eventsPath = path.join(artifactDir, 'events.ndjson');

	mkdirSync(logsDir, { recursive: true });
	writeFileSync(eventsPath, '');
	refreshLatestPointer(proofRoot, artifactDir);

	return {
		artifactDir,
		eventsPath,
		logsDir,
		record(event) {
			appendFileSync(eventsPath, `${JSON.stringify(event)}\n`);
		},
		writeSummary(summary) {
			const jsonPath = path.join(artifactDir, 'summary.json');
			const markdownPath = path.join(artifactDir, 'summary.md');
			writeFileSync(jsonPath, `${JSON.stringify(summary, null, 2)}\n`);
			writeFileSync(markdownPath, renderSummaryMarkdown(summary));
			this.record({ kind: 'artifact_written', path: jsonPath, timestamp: nowIso() });
			this.record({ kind: 'artifact_written', path: markdownPath, timestamp: nowIso() });
			return { jsonPath, markdownPath };
		},
	};
}

export function eventTimestamp(): string {
	return nowIso();
}

function refreshLatestPointer(proofRoot: string, artifactDir: string): void {
	const latestPath = path.join(proofRoot, 'latest');
	try {
		rmSync(latestPath, { force: true, recursive: true });
		symlinkSync(path.relative(proofRoot, artifactDir), latestPath, 'dir');
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		console.warn(`[proof] Warning: failed to update .proof/latest pointer: ${message}`);
	}
}

function renderSummaryMarkdown(summary: ProofSummary): string {
	const lines = [
		`# ${summary.plan.label}`,
		'',
		`Status: ${summary.status}`,
		`Duration: ${(summary.durationMs / 1000).toFixed(2)}s`,
		`Purpose: ${summary.plan.purpose}`,
		'',
		'| Step | Status | Duration | Log |',
		'| --- | --- | ---: | --- |',
	];

	for (const step of summary.steps) {
		lines.push(
			`| ${step.label} | ${step.status} | ${(step.durationMs / 1000).toFixed(2)}s | \`${step.logPath}\` |`,
		);
	}

	if (summary.failedStepId) {
		lines.push('', `Failed step: \`${summary.failedStepId}\``);
	}

	return `${lines.join('\n')}\n`;
}
