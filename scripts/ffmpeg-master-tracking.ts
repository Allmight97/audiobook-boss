/**
 * Weekly FFmpeg master tracking.
 *
 * Usage: bun scripts/ffmpeg-master-tracking.ts
 *
 * Reads the ffmpeg-revision file under vendor and updates the one issue titled
 * "Weekly FFmpeg master tracking" that github-actions[bot] opened with the
 * label ffmpeg-master-tracking. The commit paths are the fixed list below.
 * Any other issue is left alone. It does not change the pin.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

const LABEL = 'ffmpeg-master-tracking';
const TITLE = 'Weekly FFmpeg master tracking';
const BOT = 'github-actions[bot]';
const SHOWN_COMMITS = 40;
const DAY_MS = 86_400_000;
const FFMPEG_REPO = 'https://github.com/FFmpeg/FFmpeg.git';
const FFMPEG_API = 'repos/FFmpeg/FFmpeg';
// The bundled build's commit paths. The pin-move checklist names when this list changes.
const PATHSPECS = [
	'libavcodec/aac*',
	':(glob)**/*opus*',
	'libavformat/mov*',
	':(glob)**/*audiotoolbox*',
	'libavutil',
	'libswresample',
];
// FFmpeg publishes n9.1-dev while 9.1 is unreleased, so only n9.1 and n9.1.N count.
const N91_RELEASE = /^n9\.1(?:\.\d+)*$/;
const LOG_FORMAT = '%H%x1f%cI%x1f%s%x1e';

export type TagRef = { ref: string; sha: string; type: string };
export type ReleaseTag = { name: string; sha: string; type: string };
export type UpstreamCommit = { sha: string; date: string; subject: string };
export type TrackingIssue = {
	number: number;
	state: string;
	title: string;
	author: string;
	labels: string[];
};

export interface TrackingReport {
	pin: string;
	pinDate: string;
	head: string;
	headDate: string;
	masterAhead: number;
	pinNotOnMaster: number;
	daysBehind: number;
	compareUrl: string;
	releases: { name: string; sha: string }[];
	pathspecs: string[];
	commits: UpstreamCommit[];
	commitsTotal: number;
	runUrl: string;
}

interface ExecError {
	stderr?: unknown;
}

export function readPin(text: string): string {
	return commitSha(text.trim(), 'ffmpeg-revision');
}

export function daysBehind(pinDate: string, headDate: string): number {
	const pin = Date.parse(pinDate);
	const head = Date.parse(headDate);
	if (!Number.isFinite(pin) || !Number.isFinite(head)) {
		throw new Error('commit dates did not parse');
	}
	const elapsed = head - pin;
	if (elapsed <= 0) return 0;
	return Math.floor(elapsed / DAY_MS);
}

export function n91Releases(refs: TagRef[]): ReleaseTag[] {
	const releases: ReleaseTag[] = [];
	for (const ref of refs) {
		const name = ref.ref.replace(/^refs\/tags\//, '');
		if (N91_RELEASE.test(name)) releases.push({ name, sha: ref.sha, type: ref.type });
	}
	return releases;
}

export function commitsTouching(
	repo: string,
	from: string,
	to: string,
	pathspecs: string[],
	limit: number,
): { total: number; commits: UpstreamCommit[] } {
	if (pathspecs.length === 0) return { total: 0, commits: [] };
	const range = `${from}..${to}`;
	const total = wholeNumber(
		git(repo, ['rev-list', '--count', range, '--', ...pathspecs]).trim(),
		'git count',
	);
	if (limit < 1 || total === 0) return { total, commits: [] };
	const log = git(repo, [
		'log',
		'-n',
		String(limit),
		`--format=${LOG_FORMAT}`,
		range,
		'--',
		...pathspecs,
	]);
	return { total, commits: parseLog(log) };
}

export function issueToUpdate(issues: TrackingIssue[]): TrackingIssue | null {
	const ours = issues.filter(
		(issue) => issue.title === TITLE && issue.author === BOT && issue.labels.includes(LABEL),
	);
	return oldest(ours, 'OPEN') ?? oldest(ours, 'CLOSED');
}

export function renderIssue(report: TrackingReport): string {
	const lines = [
		n91Paragraph(report.releases),
		'',
		behindParagraph(report),
		'',
		"Days behind are whole 24-hour periods between the pin committer date and master HEAD's committer date.",
		'',
		'| | SHA | Committer date |',
		'| --- | --- | --- |',
		`| Pin | ${inlineCode(report.pin)} | ${report.pinDate} |`,
		`| Master HEAD | ${inlineCode(report.head)} | ${report.headDate} |`,
		'',
		`Compare API \`ahead_by\` ${report.masterAhead}, \`behind_by\` ${report.pinNotOnMaster}. ${report.compareUrl}`,
		'',
		'## Commits on the enabled libraries',
		'',
		`Paths: ${report.pathspecs.map(inlineCode).join(', ') || 'none'}.`,
		'',
		...commitLines(report),
		'',
		`Updated by [this run](${report.runUrl}). This issue is updated in place and does not move the pin.`,
		'',
	];
	return lines.join('\n');
}

function oldest(issues: TrackingIssue[], state: string): TrackingIssue | null {
	const matching = issues
		.filter((issue) => issue.state.toUpperCase() === state)
		.toSorted((a, b) => a.number - b.number);
	return matching[0] ?? null;
}

function n91Paragraph(releases: { name: string; sha: string }[]): string {
	if (releases.length === 0) {
		return '**n9.1:** not tagged. A release named `n9.1` or `n9.1.N` is flagged here. `n9.1-dev` is not a release.';
	}
	const listed = releases
		.map((release) => `${inlineCode(release.name)} at ${inlineCode(release.sha)}`)
		.join(', ');
	return `**n9.1: TAGGED** ${listed}.`;
}

function behindParagraph(report: TrackingReport): string {
	if (report.masterAhead === 0 && report.pinNotOnMaster === 0) {
		return '**Up to date** with FFmpeg master.';
	}
	const behind = `**${countNoun(report.masterAhead, 'commit')} behind**, **${countNoun(report.daysBehind, 'day')} behind** FFmpeg master.`;
	if (report.pinNotOnMaster === 0) return behind;
	return `${behind} The pin also has ${countNoun(report.pinNotOnMaster, 'commit')} absent from master.`;
}

function commitLines(report: TrackingReport): string[] {
	if (report.commitsTotal === 0) return ['No commits on these paths between the pin and master.'];
	const shown = report.commits.map(
		(commit) =>
			`- ${inlineCode(commit.sha)} ${commit.date} ${inlineCode(commit.subject)} https://github.com/FFmpeg/FFmpeg/commit/${commit.sha}`,
	);
	const note =
		report.commitsTotal > shown.length
			? `Showing ${shown.length} of ${report.commitsTotal}, newest first.`
			: `${report.commitsTotal} commits, newest first.`;
	return [...shown, '', note];
}

function countNoun(count: number, noun: string): string {
	return `${count} ${noun}${count === 1 ? '' : 's'}`;
}

function inlineCode(text: string): string {
	return `\`${text.replaceAll('`', "'")}\``;
}

function parseLog(text: string): UpstreamCommit[] {
	const commits: UpstreamCommit[] = [];
	for (const record of text.split('\x1e')) {
		const trimmed = record.trim();
		if (trimmed === '') continue;
		const [sha, date, subject] = trimmed.split('\x1f');
		if (sha === undefined || date === undefined || subject === undefined) {
			throw new Error('git log record did not parse');
		}
		commits.push({ sha: commitSha(sha, 'git log record'), date, subject });
	}
	return commits;
}

function git(repo: string, args: string[]): string {
	return execFileSync('git', ['-C', repo, ...args], {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'pipe'],
		env: childEnv(),
		maxBuffer: 10 * 1024 * 1024,
	});
}

function childEnv(): NodeJS.ProcessEnv {
	return {
		...process.env,
		GH_PROMPT_DISABLED: '1',
		GH_NO_UPDATE_NOTIFIER: '1',
		GIT_TERMINAL_PROMPT: '0',
	};
}

function main(): void {
	if (process.argv.length > 2) throw new Error('Usage: bun scripts/ffmpeg-master-tracking.ts');
	const root = path.resolve(import.meta.dirname, '..');
	const pin = readPin(
		readFileSync(soleVendorFile(root, 'ffmpeg-sys-next-', 'ffmpeg-revision'), 'utf8'),
	);
	const compare = fetchCompare(pin);
	const head = fetchHead();
	if (compare.baseSha !== pin) throw new Error('the compare base is not the pin');
	const releases = n91Releases(fetchTagRefs()).map((release) => ({
		name: release.name,
		sha: release.type === 'commit' ? release.sha : tagCommit(release.sha),
	}));
	const dir = fetchMaster(compare.baseDate);
	try {
		const localHead = git(dir, ['rev-parse', 'origin/master']).trim();
		if (localHead !== head.sha) throw new Error('fetched master is not the GitHub HEAD');
		const ahead = wholeNumber(
			git(dir, ['rev-list', '--count', `${pin}..origin/master`]).trim(),
			'git count',
		);
		if (ahead !== compare.aheadBy) {
			throw new Error(`git counts ${ahead} commits and the compare API counts ${compare.aheadBy}`);
		}
		const touched = commitsTouching(dir, pin, 'origin/master', PATHSPECS, SHOWN_COMMITS);
		const days = daysBehind(compare.baseDate, head.date);
		const body = renderIssue({
			pin,
			pinDate: compare.baseDate,
			head: head.sha,
			headDate: head.date,
			masterAhead: compare.aheadBy,
			pinNotOnMaster: compare.behindBy,
			daysBehind: days,
			compareUrl: compare.url,
			releases,
			pathspecs: PATHSPECS,
			commits: touched.commits,
			commitsTotal: touched.total,
			runUrl: runUrl(),
		});
		const url = publish(body);
		const tagged =
			releases.length === 0 ? 'not tagged' : releases.map((release) => release.name).join(', ');
		console.log(
			`issue: ${url}; commits behind: ${compare.aheadBy}; days behind: ${days}; pin: ${pin}; head: ${head.sha}; n9.1: ${tagged}`,
		);
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

function soleVendorFile(root: string, prefix: string, file: string): string {
	const found = readdirSync(path.join(root, 'vendor'))
		.filter((name) => name.startsWith(prefix))
		.map((name) => path.join(root, 'vendor', name, file))
		.filter((candidate) => existsSync(candidate));
	if (found.length !== 1 || found[0] === undefined) {
		throw new Error(`expected one vendor/${prefix}*/${file}, found ${found.length}`);
	}
	return found[0];
}

function fetchCompare(pin: string) {
	const [baseSha, baseDate, ahead, behind, url] = stringRow(
		JSON.parse(
			gh([
				'api',
				`${FFMPEG_API}/compare/${pin}...master`,
				'--jq',
				'[.base_commit.sha, .base_commit.commit.committer.date, (.ahead_by|tostring), (.behind_by|tostring), .html_url]',
			]),
		) as unknown,
		5,
		'compare API',
	);
	return {
		baseSha: commitSha(baseSha, 'compare base'),
		baseDate,
		aheadBy: wholeNumber(ahead, 'ahead_by'),
		behindBy: wholeNumber(behind, 'behind_by'),
		url,
	};
}

function fetchHead(): { sha: string; date: string } {
	const [sha, date] = stringRow(
		JSON.parse(
			gh(['api', `${FFMPEG_API}/commits/master`, '--jq', '[.sha, .commit.committer.date]']),
		) as unknown,
		2,
		'master HEAD',
	);
	return { sha: commitSha(sha, 'master HEAD'), date };
}

function fetchTagRefs(): TagRef[] {
	let body: string;
	try {
		body = gh([
			'api',
			`${FFMPEG_API}/git/matching-refs/tags/n9.1`,
			'--jq',
			'[.[] | [.ref, .object.sha, .object.type]]',
		]);
	} catch (error) {
		if (isNotFound(error)) return [];
		throw error;
	}
	const parsed = JSON.parse(body) as unknown;
	if (!Array.isArray(parsed)) throw new Error('tag list did not parse');
	return parsed.map((item) => {
		const [ref, sha, type] = stringRow(item, 3, 'tag list');
		return { ref, sha, type };
	});
}

function tagCommit(tagSha: string): string {
	const sha = gh(['api', `${FFMPEG_API}/git/tags/${tagSha}`, '--jq', '.object.sha']).trim();
	return commitSha(sha, 'n9.1 tag');
}

function stringRow(value: unknown, size: number, label: string): string[] {
	if (!Array.isArray(value) || value.length !== size) throw new Error(`${label} did not parse`);
	const row: string[] = [];
	for (const item of value) {
		if (typeof item !== 'string' || item === '') throw new Error(`${label} did not parse`);
		row.push(item);
	}
	return row;
}

function commitSha(value: string | undefined, label: string): string {
	if (value === undefined || !/^[0-9a-f]{40}$/.test(value)) {
		throw new Error(`${label} is not a commit sha`);
	}
	return value;
}

function wholeNumber(value: string | undefined, label: string): number {
	if (value === undefined || !/^\d+$/.test(value)) throw new Error(`${label} did not parse`);
	return Number(value);
}

function fetchMaster(pinDate: string): string {
	const dir = mkdtempSync(path.join(tmpdir(), 'abb-ffmpeg-'));
	const pinTime = Date.parse(pinDate);
	if (!Number.isFinite(pinTime)) throw new Error('pin date did not parse');
	const since = new Date(pinTime - DAY_MS).toISOString().slice(0, 10);
	git(dir, ['-c', 'init.defaultBranch=main', 'init', '--bare']);
	git(dir, ['remote', 'add', 'origin', FFMPEG_REPO]);
	git(dir, [
		'fetch',
		'--filter=blob:none',
		'--no-tags',
		'--shallow-since',
		since,
		'origin',
		'master',
	]);
	return dir;
}

function runUrl(): string {
	const url = process.env.TRACKING_RUN_URL ?? '';
	if (!/^https:\/\/[^\s)]+$/.test(url)) throw new Error('TRACKING_RUN_URL is not an https URL');
	return url;
}

function publish(body: string): string {
	const bodyFile = path.join(tmpdir(), `abb-ffmpeg-tracking-${process.pid}.md`);
	writeFileSync(bodyFile, body);
	try {
		ensureLabel();
		const target = issueToUpdate(listTrackingIssues());
		if (target === null) {
			return requireUrl(
				gh(['issue', 'create', '--title', TITLE, '--body-file', bodyFile, '--label', LABEL]),
			);
		}
		if (target.state.toUpperCase() === 'CLOSED') gh(['issue', 'reopen', String(target.number)]);
		gh(['issue', 'edit', String(target.number), '--body-file', bodyFile]);
		return requireUrl(
			gh(['issue', 'view', String(target.number), '--json', 'url', '--jq', '.url']),
		);
	} finally {
		rmSync(bodyFile, { force: true });
	}
}

function ensureLabel(): void {
	try {
		gh(['api', `repos/{owner}/{repo}/labels/${LABEL}`]);
	} catch (error) {
		if (!isNotFound(error)) throw error;
		gh([
			'label',
			'create',
			LABEL,
			'--description',
			'Updated in place by the FFmpeg master tracking workflow',
			'--color',
			'1d76db',
		]);
	}
}

function listTrackingIssues(): TrackingIssue[] {
	const body = gh([
		'api',
		'--paginate',
		`repos/{owner}/{repo}/issues?labels=${LABEL}&state=all&per_page=100`,
		'--jq',
		'.[] | select(.pull_request == null) | {number, state, title, author: .user.login, labels: [.labels[].name]}',
	]);
	return body.split('\n').flatMap((line) => (line === '' ? [] : [trackingIssue(JSON.parse(line))]));
}

function trackingIssue(item: unknown): TrackingIssue {
	const issue = item as TrackingIssue;
	if (
		typeof issue?.number !== 'number' ||
		typeof issue.state !== 'string' ||
		typeof issue.title !== 'string' ||
		typeof issue.author !== 'string' ||
		!Array.isArray(issue.labels)
	) {
		throw new Error('issue list did not parse');
	}
	return issue;
}

function requireUrl(text: string): string {
	const url = text
		.trim()
		.split('\n')
		.find((line) => line.startsWith('https://'));
	if (url === undefined) throw new Error('gh did not return an issue url');
	return url;
}

function gh(args: string[]): string {
	return execFileSync('gh', args, {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'pipe'],
		env: childEnv(),
		maxBuffer: 10 * 1024 * 1024,
	});
}

function isNotFound(error: unknown): boolean {
	if (typeof error !== 'object' || error === null || !('stderr' in error)) return false;
	const stderr = String((error as ExecError).stderr);
	return stderr.includes('HTTP 404') || stderr.includes('Not Found');
}

if (import.meta.main) {
	main();
}
