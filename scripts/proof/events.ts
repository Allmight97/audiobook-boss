import { appendFileSync, existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import type { ProofEvent, ProofSummary } from './types';

export type ProofArtifactWriter = {
	artifactDir: string;
	eventsPath: string;
	logsDir: string;
	discard: () => void;
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
	_repoRoot: string,
	planId: string,
	options: { artifactRoot?: string; timestamp?: string } = {},
): ProofArtifactWriter {
	const proofRoot = options.artifactRoot ?? path.join(os.tmpdir(), 'audiobook-boss-proof');
	const runsRoot = path.join(proofRoot, 'runs');
	const runPrefix = runIdPrefix(planId, options.timestamp).replace(/[:.]/g, '-');
	mkdirSync(runsRoot, { recursive: true });
	const artifactDir = mkdtempSync(path.join(runsRoot, `${runPrefix}-`));
	const logsDir = path.join(artifactDir, 'logs');
	const eventsPath = path.join(artifactDir, 'events.ndjson');

	mkdirSync(logsDir, { recursive: true });
	writeFileSync(eventsPath, '');

	return {
		artifactDir,
		eventsPath,
		logsDir,
		discard() {
			if (existsSync(artifactDir)) rmSync(artifactDir, { force: true, recursive: true });
		},
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
