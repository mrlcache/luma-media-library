// Native seeks complete asynchronously; command responses can still contain the old position.
export class NativeSeekTimeline {
    generation = 0;
    private pending: { target: number; until: number } | null = null;
    request(target: number, now = performance.now()) {
        this.generation++;
        this.pending = { target, until: now + 5000 };
        return this.generation;
    }
    position(position: number, generation: number, now = performance.now()): number | null {
        if (generation !== this.generation) return null;
        if (this.pending) {
            if (Math.abs(position - this.pending.target) > 0.65 && now < this.pending.until) return null;
            this.pending = null;
        }
        return position;
    }
    get seeking() { return this.pending !== null; }
    reset() { this.generation++; this.pending = null; }
}
