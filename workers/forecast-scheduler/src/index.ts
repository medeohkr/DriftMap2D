// index.ts triggers idle forecast workflows without duplicating source freshness checks

interface Env {
    GITHUB_TOKEN?: string;
    GITHUB_OWNER: string;
    GITHUB_REPO: string;
    GITHUB_REF: string;
}

type Fetcher = typeof fetch;
type Logger = (value: Record<string, unknown>) => void;

export const WORKFLOWS = ["norkyst_forecast.yml", "daily_forecast.yml"] as const;
const ACTIVE_STATUSES = ["queued", "in_progress", "waiting", "pending", "requested"] as const;
const API = "https://api.github.com";
const TIMEOUT_MS = 20_000;

/** query and dispatch only the configured repository without logging credentials */
async function github(env: Env, path: string, fetcher: Fetcher, body?: object) {
    const response = await fetcher(`${API}/repos/${encodeURIComponent(env.GITHUB_OWNER)}/${encodeURIComponent(env.GITHUB_REPO)}/${path}`, {
        method: body ? "POST" : "GET",
        headers: {
            "Authorization": `Bearer ${env.GITHUB_TOKEN}`,
            "Accept": "application/vnd.github+json",
            "Content-Type": "application/json",
            "User-Agent": "driftmap-forecast-scheduler",
            "X-GitHub-Api-Version": "2022-11-28",
        },
        body: body ? JSON.stringify(body) : undefined,
        // Workers supports manual redirects but not the standard error mode
        redirect: "manual",
        signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    if (!response.ok) throw new Error(`GitHub ${body ? "dispatch" : "run check"} failed with HTTP ${response.status}`);
    return response;
}

/** status-filtered queries find active runs even behind many recently completed runs */
async function checkWorkflow(env: Env, workflow: string, fetcher: Fetcher) {
    const path = `actions/workflows/${encodeURIComponent(workflow)}`;
    const checks = await Promise.all(ACTIVE_STATUSES.map(async status => {
        const query = new URLSearchParams({ branch: env.GITHUB_REF, status, per_page: "1" });
        const response = await github(env, `${path}/runs?${query}`, fetcher);
        const result = await response.json() as { workflow_runs?: { id: number }[] };
        if (!Array.isArray(result.workflow_runs)) throw new Error("GitHub returned an invalid workflow run list");
        return result.workflow_runs[0]?.id;
    }));
    const activeRun = checks.find(id => id !== undefined);
    if (activeRun !== undefined) return { workflow, outcome: "busy", run: activeRun };
    // GitHub concurrency remains the final guard if a backup run starts after the check
    await github(env, `${path}/dispatches`, fetcher, { ref: env.GITHUB_REF });
    return { workflow, outcome: "dispatched" };
}

/** one workflow failing must not prevent the other forecast from being checked */
export async function runScheduler(env: Env, fetcher: Fetcher = fetch, log: Logger = value => console.log(JSON.stringify(value))) {
    if (!env.GITHUB_TOKEN?.trim()) {
        log({ outcome: "unconfigured", error: "add the GITHUB_TOKEN Worker secret to enable dispatch" });
        return;
    }
    if (![env.GITHUB_OWNER, env.GITHUB_REPO, env.GITHUB_REF].every(value => typeof value === "string" && value.trim())) {
        throw new Error("GitHub repository and branch configuration is incomplete");
    }
    const results = await Promise.allSettled(WORKFLOWS.map(async workflow => {
        try {
            const result = await checkWorkflow(env, workflow, fetcher);
            log(result);
            return result;
        } catch (error) {
            // record only our own errors since fetch failures can contain request details
            log({ workflow, outcome: "failed", error: error instanceof Error && error.message.startsWith("GitHub ") ? error.message : "GitHub request failed" });
            throw new Error(`forecast scheduling failed for ${workflow}`);
        }
    }));
    const failures = results.filter(result => result.status === "rejected");
    if (failures.length) throw new Error(`${failures.length} forecast workflow checks failed`);
}

export default {
    async scheduled(_controller: unknown, env: Env) {
        await runScheduler(env);
    },
};
