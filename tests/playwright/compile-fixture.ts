import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export function compileFixtureToTempHtml(relativeFixturePath: string): string {
  const repoRoot = process.cwd();
  const sourcePath = path.resolve(repoRoot, relativeFixturePath);
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'gsp-playwright-'));
  const tempFixturePath = path.join(tempDir, path.basename(sourcePath));
  fs.copyFileSync(sourcePath, tempFixturePath);
  for (const extension of ['.htm', '.log']) {
    const companionPath = sourcePath.replace(/\.gsp$/i, extension);
    if (fs.existsSync(companionPath)) {
      fs.copyFileSync(
        companionPath,
        tempFixturePath.replace(/\.gsp$/i, extension),
      );
    }
  }
  execFileSync(path.resolve(repoRoot, 'target/debug/gsp-rs'), ['--html', tempFixturePath], {
    cwd: repoRoot,
    stdio: 'pipe',
  });
  return tempFixturePath.replace(/\.gsp$/i, '.html');
}
