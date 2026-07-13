import { execFileSync } from 'node:child_process';

export default function globalSetup() {
  execFileSync('cargo', ['build', '--bin', 'gsp-rs'], {
    cwd: process.cwd(),
    stdio: 'pipe',
  });
}
