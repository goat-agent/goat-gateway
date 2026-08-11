export class Locked extends Error {
  constructor() {
    super("this gateway wants its admin key");
  }
}

export class Refused extends Error {
  constructor(public readonly status: number, message: string) {
    super(message);
  }
}

let onLocked: () => void = () => {};

export function whenLocked(handler: () => void) {
  onLocked = handler;
}

export async function ask<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: {
      ...(init?.body ? { "content-type": "application/json" } : {}),
      ...init?.headers,
    },
  });

  if (response.status === 401) {
    onLocked();
    throw new Locked();
  }
  if (!response.ok) {
    throw new Refused(response.status, await complaint(response));
  }
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

export function send<T>(path: string, body: unknown, method = "POST") {
  return ask<T>(path, { method, body: JSON.stringify(body) });
}

export function drop<T>(path: string) {
  return ask<T>(path, { method: "DELETE" });
}

async function complaint(response: Response) {
  const text = await response.text();
  try {
    const parsed = JSON.parse(text) as { error?: string | { message?: string } };
    if (typeof parsed.error === "string") return parsed.error;
    if (parsed.error?.message) return parsed.error.message;
  } catch {
    // the body was not JSON, so the text itself is the best we have
  }
  return text || `the gateway answered ${response.status}`;
}

export function query(params: Record<string, string | number | undefined | null>) {
  const search = new URLSearchParams();
  for (const [name, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") {
      search.set(name, String(value));
    }
  }
  const text = search.toString();
  return text ? `?${text}` : "";
}
