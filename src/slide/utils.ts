import { invoke } from '@tauri-apps/api/core';
import type { DeckData } from '../types';

/**
 * Export presentation to PPTX via Rust backend.
 * The backend parses structured data attributes from slide HTML to build
 * proper PPTX shapes with real PowerPoint animations (not screenshots).
 */
export async function exportToPptx(deckData: DeckData): Promise<void> {
  const slides = deckData.slides.map((s, i) => ({
    html: s.html,
    index: i,
  }));

  try {
    // Call Rust PPTX builder
    const bytes = await invoke<number[]>('export_pptx', {
      title: deckData.title || 'Presentation',
      slides,
    });

    // Download the file
    const uint8 = new Uint8Array(bytes);
    const blob = new Blob([uint8], {
      type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${sanitizeFilename(deckData.title || 'presentation')}.pptx`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  } catch (err) {
    console.error('PPTX export failed:', err);
    throw new Error(`Export failed: ${err}`);
  }
}

function sanitizeFilename(name: string): string {
  return name.replace(/[<>:"/\\|?*\x00-\x1f]/g, '_').slice(0, 200);
}
