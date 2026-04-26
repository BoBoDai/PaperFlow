import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

export async function speakText(text: string, speed: number = 5): Promise<void> {
  const wpm = 100 + (speed - 1) * 22;

  try {
    await execAsync(`say -r ${wpm} "${text.replace(/"/g, '\\"')}"`);
  } catch (error) {
    throw new Error(`语音播报失败: ${error instanceof Error ? error.message : String(error)}`);
  }
}

export async function stopSpeaking(): Promise<void> {
  try {
    await execAsync('pkill -f "say -r"');
  } catch {
    // Ignore errors when stopping
  }
}
