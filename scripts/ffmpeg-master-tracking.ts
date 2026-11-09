/**
 * Weekly FFmpeg master tracking.
 *
 * Usage: bun scripts/ffmpeg-master-tracking.ts
 *
 * Reads the ffmpeg-revision file under vendor, derives the commit paths from
 * the bundled build's enabled libraries, and updates the one issue labeled
 * ffmpeg-master-tracking. It does not change the pin.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

const LABEL = 'ffmpeg-master-tracking';
const TITLE = 'Weekly FFmpeg master tracking';
const SHOWN_COMMITS = 40;
const DAY_MS = 86_400_000;
const FFMPEG_REPO = 'https://github.com/FFmpeg/FFmpeg.git';
const FFMPEG_API = 'repos/FFmpeg/FFmpeg';
// Compile flags, not libraries. build.rs does not map these to an enabled library.
const COMPILE_MODE = new Set(['build', 'build-portable', 'build-pic']);
// FFmpeg publishes n9.1-dev while 9.1 is unreleased, so only n9.1 and n9.1.N count.
const N91_RELEASE = /^n9\.1(?:\.\d+)*$/;
const LOG_FORMAT = '%H%x1f%cI%x1f%s%x1e';

export interface Library {
	name: string;
	isFeature: boolean;
}

export interface EnabledBuild {
	libraries: string[];
	externals: string[];
	compiles: boolean;
}

export interface TagRef {
	ref: string;
	sha: string;
	type: string;
}

export interface ReleaseTag {
	name: string;
	sha: string;
	type: string;
}

export interface UpstreamCommit {
	sha: string;
	date: string;
	subject: string;
}

export interface TrackingIssue {
	number: number;
	state: string;
}

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
	libraries: string[];
	externals: string[];
	pathspecs: string[];
	commits: UpstreamCommit[];
	commitsTotal: number;
	runUrl: string | null;
	generatedAt: string;
}

interface ExecError {
	stderr?: unknown;
}

export function readPin(text: string): string {
	const pin = text.trim();
	if (!/^[0-9a-f]{40}$/.test(pin)) {
		throw new Error('ffmpeg-revision is not a 40-character commit sha');
	}
	return pin;
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

export function librariesFromBuildRs(source: string): Library[] {
	const found: Library[] = [];
	for (const match of source.matchAll(/name:\s*"([^"]+)"\s*,\s*is_feature:\s*(true|false)/g)) {
		const name = match[1];
		if (name === undefined) continue;
		found.push({ name, isFeature: match[2] === 'true' });
	}
	return found;
}

export function enabledBuild(
	engineCargo: string,
	nextCargo: string,
	libraries: Library[],
): EnabledBuild {
	const table = featureTable(nextCargo);
	const enabledNext = new Set(dependencyFeatures(engineCargo, 'ffmpeg-next'));
	const bundled = featureTable(engineCargo).get('bundled-ffmpeg') ?? [];
	for (const item of bundled) {
		const name = nextFeatureName(item);
		if (name !== null) enabledNext.add(name);
	}
	closeNextFeatures(enabledNext, table);
	const sys = sysFeatures(enabledNext, table, bundled);
	return {
		libraries: libraries
			.filter((lib) => !lib.isFeature || sys.has(lib.name))
			.map((lib) => lib.name),
		externals: [...sys]
			.filter((name) => name.startsWith('build-') && !COMPILE_MODE.has(name))
			.toSorted(),
		compiles: sys.has('build'),
	};
}

export function trackingPathspecs(libraries: string[], externals: string[]): string[] {
	const enabled = new Set(libraries);
	const paths: string[] = [];
	// `*` does not match `/`, so aac and mov need the directory as well as the file glob.
	if (enabled.has('avcodec')) {
		paths.push('libavcodec/aac*', 'libavcodec/aac', ':(glob)**/*opus*');
	}
	if (enabled.has('avformat')) paths.push('libavformat/mov*', 'libavformat/mov');
	for (const feature of externals) {
		const glob = externalGlob(feature);
		if (glob !== null && !paths.includes(glob)) paths.push(glob);
	}
	for (const lib of libraries) {
		if (lib === 'avcodec' || lib === 'avformat') continue;
		paths.push(`lib${lib}`);
	}
	return paths;
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
	const total = gitCount(repo, ['rev-list', '--count', range, '--', ...pathspecs]);
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
	const open = oldest(issues, 'OPEN');
	if (open !== null) return open;
	return oldest(issues, 'CLOSED');
}

export function renderIssue(report: TrackingReport): string {
	const lines = [
		'<!-- abb-ffmpeg-master-tracking -->',
		'',
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
		`Libraries: ${codeList(report.libraries)}.`,
		`External build features: ${codeList(report.externals)}.`,
		`Paths: ${codeList(report.pathspecs)}.`,
		'',
		...commitLines(report),
		'',
		footer(report),
		'',
	];
	return lines.join('\n');
}

function featureTable(cargo: string): Map<string, string[]> {
	const table = new Map<string, string[]>();
	const body = tomlSection(cargo, 'features');
	for (const match of body.matchAll(/^([A-Za-z0-9_-]+)\s*=\s*\[/gm)) {
		const key = match[1];
		if (key === undefined) continue;
		const start = match.index + match[0].length;
		const end = body.indexOf(']', start);
		if (end < 0) throw new Error(`unclosed feature array ${key}`);
		table.set(key, quoted(body.slice(start, end)));
	}
	return table;
}

function tomlSection(text: string, name: string): string {
	const lines = text.split('\n');
	const start = lines.findIndex((line) => line.trim() === `[${name}]`);
	if (start < 0) return '';
	const rest = lines.slice(start + 1);
	const end = rest.findIndex((line) => /^\[[^\]]+\]/.test(line.trim()));
	return (end < 0 ? rest : rest.slice(0, end)).join('\n');
}

function dependencyFeatures(cargo: string, crateName: string): string[] {
	const at = cargo.search(new RegExp(`^${crateName}\\s*=`, 'm'));
	if (at < 0) return [];
	const brace = cargo.indexOf('{', at);
	const end = brace < 0 ? -1 : cargo.indexOf('}', brace);
	if (brace < 0 || end < 0 || brace - at > 80) return [];
	const features = /features\s*=\s*\[([^\]]*)\]/.exec(cargo.slice(brace, end));
	return features?.[1] === undefined ? [] : quoted(features[1]);
}

function quoted(text: string): string[] {
	return [...text.matchAll(/"([^"]+)"/g)].flatMap((match) =>
		match[1] === undefined ? [] : [match[1]],
	);
}

function nextFeatureName(dep: string): string | null {
	if (dep.startsWith('ffmpeg-next/')) return dep.slice('ffmpeg-next/'.length);
	if (!dep.includes('/')) return dep;
	return null;
}

function closeNextFeatures(enabled: Set<string>, table: Map<string, string[]>): void {
	const stack = [...enabled];
	while (stack.length > 0) {
		const name = stack.pop();
		if (name === undefined) break;
		for (const dep of table.get(name) ?? []) {
			const next = nextFeatureName(dep);
			if (next === null || enabled.has(next)) continue;
			enabled.add(next);
			stack.push(next);
		}
	}
}

function sysFeatures(
	enabledNext: Set<string>,
	table: Map<string, string[]>,
	bundled: string[],
): Set<string> {
	const sys = new Set<string>();
	for (const name of enabledNext) addSysDeps(sys, table.get(name) ?? []);
	addSysDeps(sys, bundled);
	return sys;
}

function addSysDeps(sys: Set<string>, deps: string[]): void {
	const prefix = 'ffmpeg-sys-next/';
	for (const dep of deps) {
		if (dep.startsWith(prefix)) sys.add(dep.slice(prefix.length));
	}
}

function externalGlob(feature: string): string | null {
	const name = feature.startsWith('build-lib-')
		? feature.slice('build-lib-'.length)
		: feature.slice('build-'.length);
	if (name === '' || name.includes('*') || name.includes('/')) return null;
	return `:(glob)**/*${name}*`;
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

function footer(report: TrackingReport): string {
	const via = report.runUrl === null ? '' : ` by [this run](${report.runUrl})`;
	return `Updated ${report.generatedAt}${via}. This issue is updated in place and does not move the pin.`;
}

function countNoun(count: number, noun: string): string {
	return `${count} ${noun}${count === 1 ? '' : 's'}`;
}

function codeList(items: string[]): string {
	if (items.length === 0) return 'none';
	return items.map(inlineCode).join(', ');
}

function inlineCode(text: string): string {
	return `\`${text.replaceAll('`', "'")}\``;
}

function gitCount(repo: string, args: string[]): number {
	const count = Number(git(repo, args).trim());
	if (!Number.isInteger(count)) throw new Error('git did not return an integer count');
	return count;
}

function parseLog(text: string): UpstreamCommit[] {
	const commits: UpstreamCommit[] = [];
	for (const record of text.split('\x1e')) {
		const trimmed = record.trim();
		if (trimmed === '') continue;
		const [sha, date, subject] = trimmed.split('\x1f');
		if (
			sha === undefined ||
			date === undefined ||
			subject === undefined ||
			!/^[0-9a-f]{40}$/.test(sha)
		) {
			throw new Error('git log record did not parse');
		}
		commits.push({ sha, date, subject });
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
	const pinFile = soleVendorFile(root, 'ffmpeg-sys-next-', 'ffmpeg-revision');
	const pin = readPin(readFileSync(pinFile, 'utf8'));
	const libraries = librariesFromBuildRs(
		readFileSync(path.join(path.dirname(pinFile), 'build.rs'), 'utf8'),
	);
	const enabled = enabledBuild(
		readFileSync(path.join(root, 'crates/abb-engine/Cargo.toml'), 'utf8'),
		readFileSync(soleVendorFile(root, 'ffmpeg-next-', 'Cargo.toml'), 'utf8'),
		libraries,
	);
	if (!enabled.compiles || enabled.libraries.length === 0) {
		throw new Error('the vendored build config did not enable any FFmpeg library');
	}
	const pathspecs = trackingPathspecs(enabled.libraries, enabled.externals);
	if (pathspecs.length === 0) throw new Error('the vendored build config produced no commit paths');
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
		const ahead = gitCount(dir, ['rev-list', '--count', `${pin}..origin/master`]);
		if (ahead !== compare.aheadBy) {
			throw new Error(`git counts ${ahead} commits and the compare API counts ${compare.aheadBy}`);
		}
		const touched = commitsTouching(dir, pin, 'origin/master', pathspecs, SHOWN_COMMITS);
		const body = renderIssue({
			pin,
			pinDate: compare.baseDate,
			head: head.sha,
			headDate: head.date,
			masterAhead: compare.aheadBy,
			pinNotOnMaster: compare.behindBy,
			daysBehind: daysBehind(compare.baseDate, head.date),
			compareUrl: compare.url,
			releases,
			libraries: enabled.libraries,
			externals: enabled.externals,
			pathspecs,
			commits: touched.commits,
			commitsTotal: touched.total,
			runUrl: runUrl(),
			generatedAt: new Date().toISOString(),
		});
		const url = publish(body);
		console.log(`issue: ${url}`);
		console.log(`commits behind: ${compare.aheadBy}`);
		console.log(`days behind: ${daysBehind(compare.baseDate, head.date)}`);
		console.log(`pin: ${pin}`);
		console.log(`head: ${head.sha}`);
		console.log(
			`n9.1: ${releases.length === 0 ? 'not tagged' : releases.map((release) => release.name).join(', ')}`,
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

interface ComparePayload {
	baseSha: string;
	baseDate: string;
	aheadBy: number;
	behindBy: number;
	url: string;
}

function fetchCompare(pin: string): ComparePayload {
	const body = gh([
		'api',
		`${FFMPEG_API}/compare/${pin}...master`,
		'--jq',
		'{ahead_by,behind_by,html_url,base_sha:.base_commit.sha,base_date:.base_commit.commit.committer.date}',
	]);
	const parsed = JSON.parse(body) as Partial<ComparePayload> & {
		ahead_by?: unknown;
		behind_by?: unknown;
		html_url?: unknown;
		base_sha?: unknown;
		base_date?: unknown;
	};
	if (
		typeof parsed.base_sha !== 'string' ||
		typeof parsed.base_date !== 'string' ||
		typeof parsed.html_url !== 'string' ||
		typeof parsed.ahead_by !== 'number' ||
		typeof parsed.behind_by !== 'number'
	) {
		throw new Error('compare API response is missing a count or a sha');
	}
	return {
		baseSha: parsed.base_sha,
		baseDate: parsed.base_date,
		aheadBy: parsed.ahead_by,
		behindBy: parsed.behind_by,
		url: parsed.html_url,
	};
}

function fetchHead(): { sha: string; date: string } {
	const body = gh([
		'api',
		`${FFMPEG_API}/commits/master`,
		'--jq',
		'{sha,date:.commit.committer.date}',
	]);
	const parsed = JSON.parse(body) as { sha?: unknown; date?: unknown };
	if (
		typeof parsed.sha !== 'string' ||
		typeof parsed.date !== 'string' ||
		!/^[0-9a-f]{40}$/.test(parsed.sha)
	) {
		throw new Error('master HEAD did not parse');
	}
	return { sha: parsed.sha, date: parsed.date };
}

function fetchTagRefs(): TagRef[] {
	let body: string;
	try {
		body = gh([
			'api',
			`${FFMPEG_API}/git/matching-refs/tags/n9.1`,
			'--jq',
			'[.[] | {ref, sha: .object.sha, type: .object.type}]',
		]);
	} catch (error) {
		if (isNotFound(error)) return [];
		throw error;
	}
	const parsed = JSON.parse(body) as unknown;
	if (!Array.isArray(parsed)) throw new Error('tag list did not parse');
	return parsed.flatMap((item) => {
		if (typeof item !== 'object' || item === null) return [];
		const ref = item as { ref?: unknown; sha?: unknown; type?: unknown };
		if (typeof ref.ref !== 'string' || typeof ref.sha !== 'string' || typeof ref.type !== 'string')
			return [];
		return [{ ref: ref.ref, sha: ref.sha, type: ref.type }];
	});
}

function tagCommit(tagSha: string): string {
	const sha = gh(['api', `${FFMPEG_API}/git/tags/${tagSha}`, '--jq', '.object.sha']).trim();
	if (!/^[0-9a-f]{40}$/.test(sha)) throw new Error('an n9.1 tag did not resolve to a commit');
	return sha;
}

function fetchMaster(pinDate: string): string {
	const dir = mkdtempSync(path.join(tmpdir(), 'abb-ffmpeg-'));
	const pinTime = Date.parse(pinDate);
	if (!Number.isFinite(pinTime)) throw new Error('pin date did not parse');
	const since = new Date(pinTime - DAY_MS).toISOString().slice(0, 10);
	execFileSync('git', ['-c', 'init.defaultBranch=main', '-C', dir, 'init', '--bare'], {
		stdio: ['ignore', 'pipe', 'pipe'],
		env: childEnv(),
	});
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

function runUrl(): string | null {
	const url = process.env.TRACKING_RUN_URL;
	if (url === undefined || url === '') return null;
	if (!/^https:\/\/[^\s)]+$/.test(url)) throw new Error('TRACKING_RUN_URL is not an https URL');
	return url;
}

function publish(body: string): string {
	const bodyFile = path.join(tmpdir(), `abb-ffmpeg-tracking-${process.pid}.md`);
	writeFileSync(bodyFile, body);
	try {
		gh([
			'label',
			'create',
			LABEL,
			'--description',
			'Updated in place by the FFmpeg master tracking workflow',
			'--color',
			'1d76db',
			'--force',
		]);
		const target = issueToUpdate(listTrackingIssues());
		if (target === null) {
			return requireUrl(
				gh(['issue', 'create', '--title', TITLE, '--body-file', bodyFile, '--label', LABEL]),
			);
		}
		if (target.state.toUpperCase() === 'CLOSED') gh(['issue', 'reopen', String(target.number)]);
		gh([
			'issue',
			'edit',
			String(target.number),
			'--title',
			TITLE,
			'--body-file',
			bodyFile,
			'--add-label',
			LABEL,
		]);
		return requireUrl(
			gh(['issue', 'view', String(target.number), '--json', 'url', '--jq', '.url']),
		);
	} finally {
		rmSync(bodyFile, { force: true });
	}
}

function listTrackingIssues(): TrackingIssue[] {
	const body = gh([
		'issue',
		'list',
		'--label',
		LABEL,
		'--state',
		'all',
		'--limit',
		'50',
		'--json',
		'number,state',
	]);
	const parsed = JSON.parse(body) as unknown;
	if (!Array.isArray(parsed)) throw new Error('issue list did not parse');
	return parsed.flatMap((item) => {
		if (typeof item !== 'object' || item === null) return [];
		const issue = item as { number?: unknown; state?: unknown };
		if (typeof issue.number !== 'number' || typeof issue.state !== 'string') return [];
		return [{ number: issue.number, state: issue.state }];
	});
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
