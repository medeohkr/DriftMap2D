// simulationLoop.ts advances physics once per animation frame

interface LoopOptions {
    advance(): Promise<boolean>;
    isCurrent(): boolean;
    render(force: boolean): void;
    complete(): void;
    error(error: unknown): void;
    yieldTask?: () => Promise<void>;
}

export class SimulationLoop {
    private running = false;
    private cancelled = false;
    private task: Promise<void> | null = null;
    private yieldTask: () => Promise<void>;
    private options: LoopOptions;
    private animationId: number | null = null;
    private wake: (() => void) | null = null;

    constructor(options: LoopOptions) {
        this.options = options;
        this.yieldTask = options.yieldTask ?? (() => this.waitForFrame());
    }

    /** resume an existing task so a pending WASM step can never be started twice */
    start(): Promise<void> {
        if (this.cancelled) return Promise.resolve();
        this.running = true;
        if (!this.task) this.task = this.run().finally(() => {
            this.task = null;
            this.cancelFrame();
        });
        return this.task;
    }

    pause() { this.running = false; }

    cancel() {
        this.running = false;
        this.cancelled = true;
        this.cancelFrame();
    }

    private waitForFrame(): Promise<void> {
        return new Promise(resolve => {
            this.wake = resolve;
            this.animationId = requestAnimationFrame(() => {
                this.animationId = null;
                this.wake = null;
                resolve();
            });
        });
    }

    private cancelFrame() {
        if (this.animationId !== null) cancelAnimationFrame(this.animationId);
        this.animationId = null;
        const wake = this.wake;
        this.wake = null;
        wake?.();
    }

    private current() { return !this.cancelled && this.options.isCurrent(); }

    private async run() {
        try {
            while (this.running && this.current()) {
                const finished = await this.options.advance();
                if (!this.current()) return;
                if (finished) {
                    this.running = false;
                    this.options.render(true);
                    this.options.complete();
                    return;
                }
                if (!this.running) break;
                this.options.render(false);
                await this.yieldTask();
            }
            if (this.current()) this.options.render(true);
        } catch (error) {
            this.running = false;
            if (this.current()) this.options.error(error);
        }
    }
}
