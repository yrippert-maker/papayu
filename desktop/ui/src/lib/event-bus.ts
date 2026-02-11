type EventCallback = (data?: unknown) => void | Promise<void>;

class EventBus {
  private listeners: Map<string, EventCallback[]> = new Map();

  on(event: string, callback: EventCallback): () => void {
    if (!this.listeners.has(event)) this.listeners.set(event, []);
    this.listeners.get(event)!.push(callback);
    return () => {
      const cb = this.listeners.get(event);
      if (cb) {
        const i = cb.indexOf(callback);
        if (i > -1) cb.splice(i, 1);
      }
    };
  }

  async emit(event: string, data?: unknown): Promise<void> {
    const cb = this.listeners.get(event) || [];
    await Promise.all(cb.map((fn) => fn(data)));
  }
}

export const eventBus = new EventBus();
export const Events = {
  NAVIGATE: 'navigate',
  ROUTE_CHANGED: 'route_changed',
} as const;
