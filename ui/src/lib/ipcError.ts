/**
 * Tauri serializes structured errors as plain objects. These helpers keep
 * `[object Object]` out of the UI.
 */

export function ipcErrorMessage(error: unknown): string {
  if (typeof error === 'string') return error;

  if (error && typeof error === 'object') {
    const record = error as Record<string, unknown>;
    if (typeof record.message === 'string') return record.message;
    if (typeof record.provider_message === 'string') {
      return record.provider_message;
    }
  }

  return String(error);
}

export function formatLlmError(error: unknown): string {
  const message = ipcErrorMessage(error);

  if (!error || typeof error !== 'object') return message;

  const record = error as Record<string, unknown>;
  const details: string[] = [];

  if (typeof record.status === 'number') {
    details.push(`HTTP ${record.status}`);
  }
  if (typeof record.request_id === 'string' && record.request_id) {
    details.push(`request_id: ${record.request_id}`);
  }

  return details.length > 0 ? `${message}（${details.join(' · ')}）` : message;
}
