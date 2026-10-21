import matplotlib.pyplot as plt


def plot_error_vs_step(steps, agg_pct, agg_abs, title, out_png, n_runs):
    fig, (ax_pct, ax_abs) = plt.subplots(
        2, 1, figsize=(11, 8), sharex=True
    )

    ax_pct.plot(steps, agg_pct["mean"], color="C0", lw=2, label="mean")
    ax_pct.fill_between(
        steps, agg_pct["p10"], agg_pct["p90"],
        color="C0", alpha=0.15, label="10th–90th percentile",
    )
    ax_pct.set_ylabel("centroid error (% of path length)")
    ax_pct.set_title(f"{title} ({n_runs} runs)")
    ax_pct.grid(True, alpha=0.3)
    ax_pct.legend(loc="upper left")

    ax_abs.plot(steps, agg_abs["mean"], color="C1", lw=2, label="mean")
    ax_abs.fill_between(
        steps, agg_abs["p10"], agg_abs["p90"],
        color="C1", alpha=0.15, label="10th–90th percentile",
    )
    ax_abs.set_xlabel("simulation step")
    ax_abs.set_ylabel("absolute centroid error (km)")
    ax_abs.grid(True, alpha=0.3)
    ax_abs.legend(loc="upper left")

    plt.tight_layout()
    plt.savefig(out_png, dpi=150)
    plt.close(fig)