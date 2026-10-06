export type PlayerPresentation = { active: boolean; controlsVisible: boolean };

// Serialize Android window changes so a delayed "enter" cannot override "exit".
export class PlayerPresentationQueue {
    private pending: PlayerPresentation | null = null;
    private busy = false;
    constructor(private send: (value: PlayerPresentation) => Promise<unknown>, private onError: (error: unknown) => void) {}
    update(value: PlayerPresentation) { this.pending = value; void this.flush(); }
    private async flush() {
        if (this.busy || !this.pending) return;
        const value = this.pending;
        this.pending = null;
        this.busy = true;
        try { await this.send(value); }
        catch (error) { this.onError(error); }
        finally { this.busy = false; if (this.pending) void this.flush(); }
    }
}
