# forecast scheduler

this Cloudflare Worker checks both forcing workflows every 30 minutes, at minutes 17 and 47 UTC. GitHub performs the existing forecast-reference checks and only generates tiles for newer source data. the original GitHub schedules remain as backup triggers.

the Worker checks queued, running, waiting, pending, and requested runs on `main` before dispatching each workflow. one model failing does not prevent the other from being checked. existing GitHub concurrency groups handle races with manual or backup triggers and never cancel an active generation.

## setup

create a GitHub fine-grained personal access token for `medeohkr/DriftMap2D` with repository **Actions read and write** permission. keep the token in a Cloudflare Worker secret rather than source files. no R2 or forcing-provider credentials are needed by this Worker.

run these commands from the repository root with Node 24 or newer

```sh
npm --prefix workers/forecast-scheduler test
npm --prefix workers/forecast-scheduler run test:runtime
npm --prefix workers/forecast-scheduler run check
npm --prefix workers/forecast-scheduler run deploy
npm --prefix workers/forecast-scheduler run secret:github
```

the last command prompts for the GitHub token locally and stores it directly in Cloudflare. on PowerShell, use `npm.cmd` if script execution policy blocks `npm`.

Wrangler uses the existing Cloudflare login. if none exists, run `npx --yes wrangler@4.158.0 login` from this directory before deployment. change the repository or branch in `wrangler.jsonc` when deploying for a different project.

a deployed Worker without `GITHUB_TOKEN` logs `unconfigured` and makes no GitHub requests. adding the secret activates dispatch on the next scheduled invocation. an expired or revoked token produces a failed invocation until replaced with the same secret command. public HTTP and preview endpoints are disabled.

## verification

```sh
npm --prefix workers/forecast-scheduler run logs
```

Cloudflare logs one result per workflow

- `dispatched` when an idle workflow was triggered
- `busy` with the active run ID when dispatch was skipped
- `failed` when a GitHub request failed
- `unconfigured` when the token is missing

new runs appear in GitHub Actions with the `workflow_dispatch` event. the source freshness step can exit without generating tiles when the published forecast is already current. request failures are recorded and checked again at the next scheduled invocation. ambiguous dispatch requests are not automatically retried within an invocation.

this replaces dependence on GitHub scheduled-event delivery. it does not guarantee immediate runner availability or remove the tile generation time. a source update during generation is picked up on the next timer invocation after that workflow finishes.

## maintenance

both workflows are selected by their existing file names

- `norkyst_forecast.yml` for NorKyst
- `daily_forecast.yml` for global forcing

the Worker needs no dependency installation for its unit tests. deploy and check commands use pinned Wrangler 4.158.0 through npx. inspect invocation history in Cloudflare Workers when diagnosing missed or failed dispatches. changing the cron expression can take time to propagate.

`test:runtime` starts a temporary local workerd instance and checks request construction and dispatch with mock GitHub responses. it uses no real token and triggers no workflows. this catches Workers API differences that Node-only tests cannot detect. redirect responses fail closed using `manual` mode because Workers does not implement `redirect: "error"`.
