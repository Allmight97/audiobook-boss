import { spawnSync } from 'node:child_process';
import {
	chmodSync,
	copyFileSync,
	existsSync,
	mkdirSync,
	readdirSync,
	statSync,
	unlinkSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export const aaxcleanHelperBaseName = 'abb-aaxclean-helper';

const minSidecarBytes = 1_000_000;

export function aaxcleanHelperPublishTarget(
	platform: NodeJS.Platform = process.platform,
	arch: string = process.arch,
): { rid: string; triple: string } {
	if (platform === 'darwin' && arch === 'arm64') {
		return { rid: 'osx-arm64', triple: 'aarch64-apple-darwin' };
	}
	if (platform === 'linux' && arch === 'x64') {
		return { rid: 'linux-x64', triple: 'x86_64-unknown-linux-gnu' };
	}
	if (platform === 'linux' && arch === 'arm64') {
		return { rid: 'linux-arm64', triple: 'aarch64-unknown-linux-gnu' };
	}
	throw new Error(
		`AAXClean helper publish is not supported on ${platform}/${arch}. Use macOS Apple Silicon or Linux x64/arm64.`,
	);
}

export const aaxcleanHelperTargetTriple = aaxcleanHelperPublishTarget().triple;

interface AaxcleanHelperPaths {
	projectPath: string;
	publishDir: string;
	publishedExecutablePath: string;
	sidecarDir: string;
	sidecarPath: string;
	rid: string;
	triple: string;
}

interface PublishAaxcleanHelperOptions {
	commandRunner?: typeof spawnSync;
	force?: boolean;
}

export function resolveDotnetCommand(): string {
	if (process.env.DOTNET_CLI && process.env.DOTNET_CLI.length > 0) {
		return process.env.DOTNET_CLI;
	}

	const userLocalDotnet = path.join(os.homedir(), '.dotnet', 'dotnet');
	if (existsSync(userLocalDotnet)) {
		return userLocalDotnet;
	}

	return 'dotnet';
}

export function resolveAaxcleanHelperPaths(repoRoot: string): AaxcleanHelperPaths {
	const { rid, triple } = aaxcleanHelperPublishTarget();
	const projectPath = path.join(
		repoRoot,
		'tools/abb-aaxclean-helper/src/AbbAaxcleanHelper/AbbAaxcleanHelper.csproj',
	);
	const publishDir = path.join(repoRoot, 'tools/abb-aaxclean-helper/.publish', rid);
	const sidecarDir = path.join(repoRoot, 'src-tauri/binaries');
	return {
		projectPath,
		publishDir,
		publishedExecutablePath: path.join(publishDir, aaxcleanHelperBaseName),
		sidecarDir,
		sidecarPath: path.join(sidecarDir, `${aaxcleanHelperBaseName}-${triple}`),
		rid,
		triple,
	};
}

export function publishAaxcleanHelper(
	repoRoot: string,
	options: PublishAaxcleanHelperOptions = {},
): string {
	const paths = resolveAaxcleanHelperPaths(repoRoot);
	if (!options.force && helperSidecarIsFresh(repoRoot, paths.sidecarPath)) {
		console.log(`[aaxclean-helper] Using existing ${paths.sidecarPath}`);
		return paths.sidecarPath;
	}

	if (existsSync(paths.publishedExecutablePath)) {
		unlinkSync(paths.publishedExecutablePath);
	}

	const dotnet = resolveDotnetCommand();
	const result = (options.commandRunner ?? spawnSync)(
		dotnet,
		['publish', paths.projectPath, '-c', 'Release', '-r', paths.rid, '-o', paths.publishDir],
		{
			cwd: repoRoot,
			stdio: 'inherit',
		},
	);

	if (result.error) {
		throw result.error;
	}
	if (result.status !== 0) {
		const detail =
			typeof result.status === 'number'
				? `status ${result.status}`
				: result.signal
					? `signal ${result.signal}`
					: 'no successful exit status';
		throw new Error(`AAXClean helper publish failed (${detail})`);
	}
	if (!existsSync(paths.publishedExecutablePath)) {
		throw new Error(`Expected published AAXClean helper at ${paths.publishedExecutablePath}`);
	}

	mkdirSync(paths.sidecarDir, { recursive: true });
	copyFileSync(paths.publishedExecutablePath, paths.sidecarPath);
	chmodSync(paths.sidecarPath, 0o755);
	console.log(`[aaxclean-helper] Published ${paths.sidecarPath}`);
	return paths.sidecarPath;
}

export function verifyAaxcleanHelperSidecar(repoRoot: string): void {
	const { sidecarPath } = resolveAaxcleanHelperPaths(repoRoot);
	if (!existsSync(sidecarPath)) {
		throw new Error(`Expected AAXClean helper sidecar at ${sidecarPath}`);
	}
}

function helperSidecarIsFresh(repoRoot: string, sidecarPath: string): boolean {
	if (!existsSync(sidecarPath)) {
		return false;
	}
	// A leftover `exit 0` stub is executable and newer than sources; size
	// is what distinguishes it from a real self-contained helper.
	if (statSync(sidecarPath).size < minSidecarBytes) {
		return false;
	}
	const sidecarMtime = statSync(sidecarPath).mtimeMs;
	return latestHelperSourceMtime(repoRoot) <= sidecarMtime;
}

function latestHelperSourceMtime(repoRoot: string): number {
	const helperRoot = path.join(repoRoot, 'tools/abb-aaxclean-helper');
	let latest = 0;
	for (const filePath of walk(helperRoot)) {
		if (!helperSourceAffectsPublish(filePath)) {
			continue;
		}
		latest = Math.max(latest, statSync(filePath).mtimeMs);
	}
	return latest;
}

function helperSourceAffectsPublish(filePath: string): boolean {
	return (
		filePath.endsWith('.cs') ||
		filePath.endsWith('.csproj') ||
		filePath.endsWith('.props') ||
		filePath.endsWith('.targets') ||
		path.basename(filePath) === 'global.json'
	);
}

function* walk(directory: string): Generator<string> {
	for (const entry of readdirSync(directory, { withFileTypes: true })) {
		const fullPath = path.join(directory, entry.name);
		if (entry.isDirectory()) {
			if (entry.name === 'bin' || entry.name === 'obj' || entry.name === '.publish') {
				continue;
			}
			yield* walk(fullPath);
		} else {
			yield fullPath;
		}
	}
}

if (import.meta.main) {
	const repoRoot = path.resolve(import.meta.dir, '..');
	publishAaxcleanHelper(repoRoot);
}
