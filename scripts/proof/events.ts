import { appendFileSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
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

function runId(planId: string): string {
	const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
	const safePlanId = planId.replace(/[^a-zA-Z0-9._-]/g, '-');
	return `${timestamp}-${safePlanId}`;
}

export function createArtifactWriter(repoRoot: string, planId: string): ProofArtifactWriter {
	const proofRoot = path.join(repoRoot, '.proof');
	const artifactDir = path.join(proofRoot, 'runs', runId(planId));
	const logsDir = path.join(artifactDir, 'logs');
	const eventsPath = path.join(artifactDir, 'events.ndjson');

	mkdirSync(path.join(proofRoot, 'runs'), { recursive: true });
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
	rmSync(latestPath, { force: true, recursive: true });
	symlinkSync(path.relative(proofRoot, artifactDir), latestPath, 'dir');
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
