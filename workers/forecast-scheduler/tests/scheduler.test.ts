// scheduler.test.ts checks duplicate prevention and independent forecast dispatch

import assert from "node:assert/strict";
import test from "node:test";
import { runScheduler, WORKFLOWS } from "../src/index.ts";

const env = { GITHUB_TOKEN: "test-secret", GITHUB_OWNER: "medeohkr", GITHUB_REPO: "DriftMap2D", GITHUB_REF: "main" };
type Call = { url: URL; init: RequestInit };

function mockGitHub(respond: (call: Call) => Response = () => Response.json({ workflow_runs: [] })) {
    const calls: Call[] = [];
    const fetcher = (async (input: string | URL | Request, init: RequestInit = {}) => {
        const call = { url: new URL(String(input)), init };
        calls.push(call);
        if (init.method === "POST") return new Response(null, { status: 204 });
        return respond(call);
    }) as typeof fetch;
    return { calls, fetcher };
}

const quiet = () => {};

test("idle NorKyst and global workflows dispatch main with authenticated requests", async () => {
    const { calls, fetcher } = mockGitHub();
    await runScheduler(env, fetcher, quiet);
    const dispatches = calls.filter(c => c.init.method === "POST");
    assert.equal(dispatches.length, 2);
    for (const workflow of WORKFLOWS) {
        const call = dispatches.find(c => c.url.pathname.endsWith(`/${workflow}/dispatches`))!;
        assert.deepEqual(JSON.parse(String(call.init.body)), { ref: "main" });
    }
    for (const call of calls) {
        assert.equal(call.url.origin, "https://api.github.com");
        assert.ok(call.url.pathname.startsWith("/repos/medeohkr/DriftMap2D/actions/workflows/"));
        assert.equal(new Headers(call.init.headers).get("Authorization"), "Bearer test-secret");
        assert.ok(call.init.signal);
        assert.equal(call.init.redirect, "manual");
        if (call.init.method === "GET") {
            assert.equal(call.url.searchParams.get("branch"), "main");
            assert.equal(call.url.searchParams.get("per_page"), "1");
        }
    }
});

test("every active status prevents dispatch without blocking the other model", async () => {
    for (const status of ["queued", "in_progress", "waiting", "pending", "requested"]) {
        const logs: Record<string, unknown>[] = [];
        const { calls, fetcher } = mockGitHub(call => Response.json({
            workflow_runs: call.url.pathname.includes("norkyst_forecast.yml") && call.url.searchParams.get("status") === status ? [{ id: 42 }] : [],
        }));
        await runScheduler(env, fetcher, value => logs.push(value));
        const dispatches = calls.filter(c => c.init.method === "POST");
        assert.equal(dispatches.length, 1);
        assert.ok(dispatches[0].url.pathname.includes("daily_forecast.yml"));
        assert.ok(logs.some(value => value.outcome === "busy" && value.run === 42));
    }
});

test("failed run checks do not dispatch that model or suppress the other model", async () => {
    const logs: Record<string, unknown>[] = [];
    const { calls, fetcher } = mockGitHub(call => call.url.pathname.includes("norkyst_forecast.yml")
        ? Response.json({ message: "denied" }, { status: 403 }) : Response.json({ workflow_runs: [] }));
    await assert.rejects(runScheduler(env, fetcher, value => logs.push(value)), /1 forecast workflow checks failed/);
    assert.equal(calls.filter(c => c.init.method === "POST").length, 1);
    assert.ok(calls.find(c => c.init.method === "POST")!.url.pathname.includes("daily_forecast.yml"));
    assert.ok(logs.some(value => String(value.error).includes("HTTP 403")));
    assert.ok(!JSON.stringify(logs).includes(env.GITHUB_TOKEN));
});

test("malformed run lists fail closed instead of assuming a workflow is idle", async () => {
    const { calls, fetcher } = mockGitHub(call => Response.json(call.url.pathname.includes("norkyst_forecast.yml") ? {} : { workflow_runs: [] }));
    await assert.rejects(runScheduler(env, fetcher, quiet));
    assert.equal(calls.filter(c => c.init.method === "POST").length, 1);
    assert.ok(!calls.some(c => c.init.method === "POST" && c.url.pathname.includes("norkyst_forecast.yml")));
});

test("a rejected dispatch is reported without retrying an ambiguous POST", async () => {
    const { calls, fetcher } = mockGitHub();
    const failing = (async (input: string | URL | Request, init?: RequestInit) => {
        const response = await fetcher(input, init);
        if (init?.method === "POST" && String(input).includes("norkyst_forecast.yml")) return new Response(null, { status: 422 });
        return response;
    }) as typeof fetch;
    await assert.rejects(runScheduler(env, failing, quiet));
    assert.equal(calls.filter(c => c.init.method === "POST").length, 2);
});

test("missing credentials leave the deployed scheduler inert and log setup instructions", async () => {
    const logs: Record<string, unknown>[] = [];
    await runScheduler({ ...env, GITHUB_TOKEN: undefined }, (async () => { throw new Error("unexpected request"); }) as typeof fetch, value => logs.push(value));
    assert.equal(logs[0].outcome, "unconfigured");
});

test("invalid configuration fails before any GitHub requests", async () => {
    await assert.rejects(runScheduler({ ...env, GITHUB_REF: "" }, (async () => { throw new Error("unexpected request"); }) as typeof fetch, quiet), /configuration is incomplete/);
});

test("unexpected network errors never expose request details in logs", async () => {
    const logs: Record<string, unknown>[] = [];
    await assert.rejects(runScheduler(env, (async () => { throw new Error(`request failed with Bearer ${env.GITHUB_TOKEN}`); }) as typeof fetch, value => logs.push(value)));
    assert.equal(logs.length, 2);
    assert.ok(!JSON.stringify(logs).includes(env.GITHUB_TOKEN));
});


test("redirects fail closed without forwarding credentials or dispatching twice", async () => {
    const { calls, fetcher } = mockGitHub(() => new Response(null, {
        status: 302, headers: { Location: "https://example.invalid/" },
    }));
    await assert.rejects(runScheduler(env, fetcher, quiet));
    assert.ok(!calls.some(call => call.init.method === "POST"));
    assert.ok(calls.every(call => call.init.redirect === "manual"));
});
