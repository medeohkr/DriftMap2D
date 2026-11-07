// runtime-worker.ts exercises real Workers request construction with mock GitHub responses

import { runScheduler } from "../src/index.ts";

export default {
    async fetch(request: Request) {
        const redirect = new URL(request.url).pathname === "/redirect";
        const calls: string[] = [];
        const env = { GITHUB_TOKEN: "runtime-test-placeholder", GITHUB_OWNER: "medeohkr", GITHUB_REPO: "DriftMap2D", GITHUB_REF: "main" };
        try {
            await runScheduler(env, async (url, init) => {
                // this constructor rejects unsupported options in workerd rather than Node
                const outgoing = new Request(url, init);
                calls.push(outgoing.method);
                if (redirect) return new Response(null, { status: 302, headers: { Location: "https://example.invalid/" } });
                return outgoing.method === "POST" ? new Response(null, { status: 204 }) : Response.json({ workflow_runs: [] });
            }, () => {});
            if (redirect) throw new Error("unexpected success on redirect");
        } catch (error) {
            if (!redirect || calls.length !== 10 || calls.includes("POST")) throw error;
        }
        const dispatches = calls.filter(method => method === "POST").length;
        if (!redirect && (calls.length !== 12 || dispatches !== 2)) throw new Error("unexpected dispatch count");
        return Response.json({ passed: true, dispatches });
    },
};
