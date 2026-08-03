// Module-level singleton that survives React component unmount/remount.
// Tracks an in-flight AI response so navigation away from ChatPage
// does not lose the pending response.

import type { Message } from '../api';

export interface InflightMessage {
  id: string;
  role: 'user' | 'assistant';
  content: string;
  created_at?: string;
  pending?: boolean;
}

interface InflightChat {
  threadId: string;
  messagesBefore: InflightMessage[];
  userMsg: InflightMessage;
  promise: Promise<{ assistant_message: Message }>;
}

let current: InflightChat | null = null;

export function setInflight(c: InflightChat): void {
  current = c;
}

export function getInflight(): InflightChat | null {
  return current;
}

export function clearInflight(): void {
  current = null;
}
