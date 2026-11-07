// runtime-check.mjs validates the scheduler in workerd without real GitHub credentials

import assert from "node:assert/strict";
import { spawn, execFile } from "node:child_process";
import { createServer } from "node:net";
import { setTimeout as pause } from "node:timers/promises";

const server = createServer();
await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
const port = server.address().port;
await new Promise(resolve => server.close(resolve));
assert.ok(process.env.npm_execpath, "run this check through npm run test:runtime");
const child = spawn(process.execPath, [process.env.npm_execpath, "exec", "--yes", "--package=wrangler@4.158.0", "--",
    "wrangler", "dev", "--local", "--config", "tests/runtime.wrangler.jsonc", "--port", String(port)],
    { windowsHide: true, stdio: ["ignore", "pipe", "pipe"] });
let output = "";
child.stdout.on("data", value => { output += value; });
child.stderr.on("data", value => { output += value; });
try {
    let response;
    for (let i = 0; i < 240; i++) {
        try { response = await fetch(`http://127.0.0.1:${port}/`, { signal: AbortSignal.timeout(2000) }); break; }
        catch {
            if (child.exitCode !== null) throw new Error(output);
            await pause(250);
        }
    }
    assert.ok(response, `runtime did not start\n${output}`);
    assert.equal(response.status, 200, await response.clone().text());
    assert.deepEqual(await response.json(), { passed: true, dispatches: 2 });
    const redirect = await fetch(`http://127.0.0.1:${port}/redirect`);
    assert.equal(redirect.status, 200, await redirect.clone().text());
    assert.deepEqual(await redirect.json(), { passed: true, dispatches: 0 });
    console.log("workerd checks passed for dispatch and redirect rejection");
} finally {
    if (child.exitCode === null) {
        if (process.platform === "win32") await new Promise(resolve => execFile("taskkill", ["/PID", String(child.pid), "/T", "/F"], { windowsHide: true }, resolve));
        else child.kill("SIGTERM");
    }
}
