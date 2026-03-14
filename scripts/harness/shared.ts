import { copyFile, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';

import { createServer, type ViteDevServer } from 'vite';
import type { ConsoleMessage, Page } from 'playwright';

export const HARNESS_HOST = '127.0.0.1';
export const DEFAULT_HARNESS_PORT = 4173;
export const HARNESS_ARTIFACT_ROOT = path.resolve('.artifacts/harness');

export type HarnessServerSession = {
	server: ViteDevServer;
	port: number;
	origin: string;
};

export type HarnessConsoleMessage = {
	type: string;
	text: string;
};

export async function writeJsonArtifact(filePath: string, payload: unknown): Promise<void> {
	await mkdir(path.dirname(filePath), { recursive: true });
	await writeFile(filePath, `${JSON.stringify(payload, null, 2)}\n`, 'utf8');
}

export async function mirrorArtifactToLatest(
	sourcePath: string,
	latestPath: string,
): Promise<string> {
	await mkdir(path.dirname(latestPath), { recursive: true });
	await copyFile(sourcePath, latestPath);
	return latestPath;
}

export async function startHarnessServer(): Promise<HarnessServerSession> {
	const server = await createServer({
		server: {
			host: HARNESS_HOST,
			port: DEFAULT_HARNESS_PORT,
			strictPort: false,
		},
	});
	await server.listen();
	const address = server.httpServer?.address();
	if (!address || typeof address === 'string') {
		await server.close();
		throw new Error('Harness server did not expose a numeric local port.');
	}

	return {
		server,
		port: address.port,
		origin: `http://${HARNESS_HOST}:${address.port}`,
	};
}

export async function gotoHarnessRoute(page: Page, origin: string, route: string): Promise<void> {
	await page.goto(`${origin}${route}`, {
		waitUntil: 'load',
	});
	await page.waitForFunction(
		() => typeof window.__ABB_HARNESS__ !== 'undefined' && window.__ABB_HARNESS_READY__ === true,
		{
			timeout: 10_000,
		},
	);
}

export async function resetHarnessState(page: Page): Promise<void> {
	await page.evaluate(async () => {
		await window.__ABB_HARNESS__?.reset();
	});
}

export function summarizeConsoleMessage(message: ConsoleMessage): HarnessConsoleMessage {
	return {
		type: message.type(),
		text: message.text(),
	};
}
