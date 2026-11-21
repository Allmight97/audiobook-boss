import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const FFMPEG_SYS = 'vendor/ffmpeg-sys-next-9.0.1';

const HELP = `Usage: bun scripts/check-bundled-licenses.ts [app-path]

Checks that the built app ships every file in src-tauri/tauri.conf.json
bundle.resources, byte for byte, and that the FFmpeg notice names the pinned
commit. With no app path, checks target/release/bundle/macos/<productName>.app.`;

export interface TauriBundle {
	bundle?: { resources?: unknown };
	productName?: string;
}

export function bundleResources(config: TauriBundle): [string, string][] {
	const resources = config.bundle?.resources;
	if (!resources || typeof resources !== 'object' || Array.isArray(resources)) {
		throw new Error(
			'src-tauri/tauri.conf.json bundle.resources must map each source to its target.',
		);
	}
	return Object.entries(resources as Record<string, string>);
}

export function bundleResourceProblems(
	resources: [string, string][],
	srcTauriDir: string,
	resourcesDir: string,
): string[] {
	return resources.flatMap(([source, target]) => {
		const shipped = path.join(resourcesDir, target);
		if (!existsSync(shipped)) return [`${target} is missing (from ${source})`];
		const expected = readFileSync(path.resolve(srcTauriDir, source));
		return readFileSync(shipped).equals(expected) ? [] : [`${target} differs from ${source}`];
	});
}

export function ffmpegNoticeProblem(
	resources: [string, string][],
	resourcesDir: string,
	revision: string,
): string | undefined {
	const target = resources.find(([source]) => source === `../${FFMPEG_SYS}/ABB-PROVENANCE.md`)?.[1];
	if (!target) return `bundle.resources ships no ${FFMPEG_SYS}/ABB-PROVENANCE.md`;
	const shipped = path.join(resourcesDir, target);
	if (!existsSync(shipped)) return undefined;
	return readFileSync(shipped, 'utf8').includes(revision)
		? undefined
		: `${target} does not name the pinned FFmpeg commit ${revision}`;
}

function bundledLicenseProblems(repoRoot: string, app: string): string[] {
	const config = JSON.parse(
		readFileSync(path.join(repoRoot, 'src-tauri/tauri.conf.json'), 'utf8'),
	) as TauriBundle;
	const resources = bundleResources(config);
	const resourcesDir = path.join(app, 'Contents/Resources');
	const revision = readFileSync(path.join(repoRoot, FFMPEG_SYS, 'ffmpeg-revision'), 'utf8').trim();
	return [
		...bundleResourceProblems(resources, path.join(repoRoot, 'src-tauri'), resourcesDir),
		ffmpegNoticeProblem(resources, resourcesDir, revision),
	].filter((problem) => problem !== undefined);
}

function defaultApp(repoRoot: string): string {
	const config = JSON.parse(
		readFileSync(path.join(repoRoot, 'src-tauri/tauri.conf.json'), 'utf8'),
	) as TauriBundle;
	if (!config.productName) {
		throw new Error('src-tauri/tauri.conf.json has no productName.');
	}
	return path.join(repoRoot, 'target/release/bundle/macos', `${config.productName}.app`);
}

function main(): void {
	if (process.argv.includes('--help') || process.argv.includes('-h')) {
		console.log(HELP);
		return;
	}
	const repoRoot = path.resolve(import.meta.dirname, '..');
	const app = process.argv[2] ? path.resolve(process.argv[2]) : defaultApp(repoRoot);
	const problems = bundledLicenseProblems(repoRoot, app);
	if (problems.length > 0) {
		throw new Error(`${app} does not ship its license notices:\n- ${problems.join('\n- ')}`);
	}
	console.log(`Licenses in ${app} match tauri.conf.json bundle.resources.`);
}

if (import.meta.main) {
	try {
		main();
	} catch (error) {
		console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
		process.exit(1);
	}
}
