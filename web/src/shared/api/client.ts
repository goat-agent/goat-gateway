import { useSession } from "@/shared/model/session";

export class Refused extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}

export async function ask<T>(path: string): Promise<T> {
  const response = await fetch(path);
  await checked(response);
  return response.json();
}

export async function send<T>(path: string, body: unknown): Promise<T> {
  return (await posted(path, body)).json();
}

export async function tell(path: string, body: unknown): Promise<void> {
  await posted(path, body);
}

async function posted(path: string, body: unknown) {
  const response = await fetch(path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  await checked(response);
  return response;
}

export async function drop(path: string): Promise<void> {
  const response = await fetch(path, { method: "DELETE" });
  await checked(response);
}

async function checked(response: Response) {
  if (response.status === 401) {
    useSession.getState().lock();
    throw new Refused(401, "this gateway wants its admin key");
  }
  if (!response.ok) {
    throw new Refused(response.status, await complaint(response));
  }
}

async function complaint(response: Response) {
  const text = await response.text();
  return spoken(parsed(text)) ?? text ?? `the gateway answered ${response.status}`;
}

function parsed(text: string): unknown {
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

function spoken(body: unknown): string | undefined {
  if (typeof body !== "object" || body === null) return undefined;
  if ("error" in body) {
    const { error } = body;
    if (typeof error === "string") return error;
    if (typeof error === "object" && error !== null && "message" in error) {
      const { message } = error;
      if (typeof message === "string") return message;
    }
  }
  if ("message" in body && typeof body.message === "string") return body.message;
  return undefined;
}

export function query(params: Record<string, string | number | undefined>) {
  const search = new URLSearchParams();
  for (const [name, value] of Object.entries(params)) {
    if (value !== undefined && value !== "") search.set(name, String(value));
  }
  const text = search.toString();
  return text ? `?${text}` : "";
}
