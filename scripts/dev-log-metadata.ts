export interface MetadataStageEvent {
	stage: string;
	status: string;
	artifact: string;
	elapsedMs?: number;
	strategy?: string;
	fs?: string;
	bytesWritten?: number;
	bytesRead?: number;
	syncMs?: number;
	chmodMs?: number;
	renameMs?: number;
	closeMs?: number;
	mdatShifted?: string;
	layoutBefore?: string;
	layoutAfter?: string;
	mode?: string;
	skipped?: string;
}

const INTERESTING_STAGES = new Set([
	'metadata_classify',
	'metadata_plan_read',
	'metadata_copy_source',
	'metadata_mp4_tags',
	'metadata_remux',
	'metadata_verify_audio',
	'metadata_replace',
	'metadata_open_input',
	'metadata_create_output',
	'metadata_copy_packets',
]);

function parseTokens(rest: string): Record<string, string> {
	const fields: Record<string, string> = {};
	for (const token of rest.trim().split(/\s+/)) {
		const eq = token.indexOf('=');
		if (eq <= 0 || eq === token.length - 1) continue;
		fields[token.slice(0, eq)] = token.slice(eq + 1);
	}
	return fields;
}

function optionalInt(value: string | undefined): number | undefined {
	if (!value || !/^\d+$/.test(value)) return undefined;
	const parsed = Number.parseInt(value, 10);
	return Number.isSafeInteger(parsed) ? parsed : undefined;
}

function pickDetail(event: MetadataStageEvent): string {
	const parts: string[] = [];
	if (event.strategy) parts.push(`strategy=${event.strategy}`);
	if (event.fs) parts.push(`fs=${event.fs}`);
	if (event.mode) parts.push(`mode=${event.mode}`);
	if (event.skipped) parts.push(`skipped=${event.skipped}`);
	if (event.bytesRead !== undefined) parts.push(`read=${event.bytesRead}`);
	if (event.layoutBefore && event.layoutAfter) {
		parts.push(`layout=${event.layoutBefore}->${event.layoutAfter}`);
	}
	if (event.mdatShifted) parts.push(`mdat_shifted=${event.mdatShifted}`);
	if (event.closeMs !== undefined) parts.push(`close=${event.closeMs}ms`);
	if (event.bytesWritten !== undefined) parts.push(`written=${event.bytesWritten}`);
	if (event.syncMs !== undefined) parts.push(`sync=${event.syncMs}ms`);
	if (event.chmodMs !== undefined) parts.push(`chmod=${event.chmodMs}ms`);
	if (event.renameMs !== undefined) parts.push(`rename=${event.renameMs}ms`);
	return parts.length > 0 ? parts.join(' ') : '—';
}

function tableCell(value: string | number | undefined): string {
	return String(value ?? '—').replaceAll('|', '\\|');
}

export function parseMetadataStageLine(line: string): MetadataStageEvent | null {
	const marker = 'media_stage ';
	const index = line.indexOf(marker);
	if (index < 0) return null;
	const fields = parseTokens(line.slice(index + marker.length));
	const stage = fields.stage;
	const status = fields.status;
	if (!stage || !status || status === 'start' || !INTERESTING_STAGES.has(stage)) {
		return null;
	}
	return {
		stage,
		status,
		artifact: fields.artifact ?? '—',
		elapsedMs: optionalInt(fields.elapsed_ms),
		strategy: fields.strategy,
		fs: fields.fs,
		bytesWritten: optionalInt(fields.bytes_written),
		bytesRead: optionalInt(fields.bytes_read),
		syncMs: optionalInt(fields.sync_ms),
		chmodMs: optionalInt(fields.chmod_ms),
		renameMs: optionalInt(fields.rename_ms),
		closeMs: optionalInt(fields.close_ms),
		mdatShifted: fields.mdat_shifted,
		layoutBefore: fields.layout_before,
		layoutAfter: fields.layout_after,
		mode: fields.mode,
		skipped: fields.skipped,
	};
}

export function renderMetadataStages(stages: readonly MetadataStageEvent[]): string[] {
	if (stages.length === 0) return ['No completed metadata stages were captured.'];
	const recent = stages.slice(-40);
	return [
		'| Stage | Artifact | Status | Elapsed | Details |',
		'| --- | --- | --- | ---: | --- |',
		...recent.map((event) => {
			const elapsed = event.elapsedMs === undefined ? '—' : `${event.elapsedMs}ms`;
			return [
				`| ${tableCell(event.stage)}`,
				`\`${tableCell(event.artifact)}\``,
				tableCell(event.status),
				tableCell(elapsed),
				`${tableCell(pickDetail(event))} |`,
			].join(' | ');
		}),
	];
}
