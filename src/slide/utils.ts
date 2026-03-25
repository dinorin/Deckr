import JSZip from 'jszip';
import html2canvas from 'html2canvas';
import PptxGenJS from 'pptxgenjs';
import { Chart, registerables } from 'chart.js';
import { createIcons, icons as lucideIcons } from 'lucide';
import { save } from '@tauri-apps/plugin-dialog';
import { writeFile } from '@tauri-apps/plugin-fs';
import type { DeckData } from '../types';

Chart.register(...registerables);

const SLIDE_W_PX = 960;
const SLIDE_H_PX = 540;
const PPTX_W_IN = 13.33; // LAYOUT_WIDE
const PPTX_H_IN = 7.5;

function pxXtoIn(px: number) { return (px / SLIDE_W_PX) * PPTX_W_IN; }
function pxYtoIn(px: number) { return (px / SLIDE_H_PX) * PPTX_H_IN; }

// ── Style baking ──────────────────────────────────────────────────────────────

const COLOR_PROPS = [
  'color', 'background-color',
  'border-top-color', 'border-right-color', 'border-bottom-color', 'border-left-color',
  'outline-color', 'box-shadow', 'text-shadow',
];

function bakeStyles(root: HTMLElement) {
  for (const el of Array.from(root.querySelectorAll<HTMLElement>('*')).concat([root])) {
    const cs = window.getComputedStyle(el);
    for (const prop of COLOR_PROPS) {
      const val = cs.getPropertyValue(prop);
      if (val && (val.includes('oklab') || val.includes('oklch'))) {
        el.style.setProperty(prop, val);
      }
    }
  }
}

// ── Image proxy ───────────────────────────────────────────────────────────────

import { invoke } from '@tauri-apps/api/core';

function fetchWithTimeout(url: string): Promise<string> {
  return Promise.race([
    invoke<string>('fetch_image_base64', { url }),
    new Promise<string>((_, reject) => setTimeout(() => reject('timeout'), 3000)),
  ]);
}

async function inlineImages(container: HTMLElement) {
  const tasks: Promise<void>[] = [];

  for (const img of Array.from(container.querySelectorAll<HTMLImageElement>('img[src]'))) {
    const src = img.getAttribute('src');
    if (src?.startsWith('http')) {
      tasks.push(
        fetchWithTimeout(src)
          .then(data => { img.src = data; })
          .catch(() => { img.removeAttribute('src'); })
      );
    }
  }

  for (const el of Array.from(container.querySelectorAll<HTMLElement>('[style*="background-image"]'))) {
    const match = (el.getAttribute('style') ?? '').match(/url\(['"]?(https?[^'")\s]+)['"]?\)/);
    if (match) {
      tasks.push(
        fetchWithTimeout(match[1])
          .then(data => { el.style.backgroundImage = `url("${data}")`; })
          .catch(() => { el.style.backgroundImage = 'none'; })
      );
    }
  }

  await Promise.allSettled(tasks);
}

// ── Canvas options ────────────────────────────────────────────────────────────

const CANVAS_OPTS = {
  scale: 2,
  useCORS: true,
  allowTaint: true,
  logging: false,
  backgroundColor: null as null,
  onclone: (clonedDoc: Document) => {
    for (const el of Array.from(clonedDoc.querySelectorAll('style, link[rel="stylesheet"]'))) {
      el.remove();
    }
  },
};

// ── Text overflow fitter ──────────────────────────────────────────────────────

/**
 * Scan every layer-text-* wrapper inside `slideEl`. If any wrapper's content
 * overflows its fixed dimensions (vertically for wrapping text, horizontally for
 * nowrap single-line text), reduce all [data-ppt-font-size] children's font
 * sizes proportionally — stepping down 5 % at a time — until the overflow is
 * gone or we reach `minFontPx`. Updates both inline `style.fontSize` (visual)
 * and `data-ppt-font-size` attribute (PPTX export).
 */
export function fitTextToLayers(slideEl: HTMLElement, minFontPx = 7): void {
  for (const wrapper of Array.from(slideEl.querySelectorAll<HTMLElement>('[id^="layer-text-"]'))) {
    const textEls = Array.from(wrapper.querySelectorAll<HTMLElement>('[data-ppt-font-size]'));
    if (textEls.length === 0) continue;

    // Snapshot sizes from the data attribute (source of truth set by the agent)
    const baseSizes = textEls.map(
      el => Math.max(1, parseInt(el.getAttribute('data-ppt-font-size') || '0', 10)
                       || parseFloat(window.getComputedStyle(el).fontSize) || 16)
    );

    const isOverflowing = (): boolean => {
      // Vertical overflow (multi-line wrapping text)
      if (wrapper.scrollHeight > wrapper.clientHeight + 1) return true;
      // Horizontal overflow (white-space:nowrap single-line elements)
      for (const el of textEls) {
        if (el.scrollWidth > el.clientWidth + 1) return true;
      }
      return false;
    };

    if (!isOverflowing()) continue;

    // Binary-search the largest scale factor that fits, bounded by minFontPx
    const maxBaseSize = Math.max(...baseSizes);
    const minScale    = minFontPx / maxBaseSize;

    const applyScale = (scale: number) => {
      textEls.forEach((el, i) => {
        const sz = Math.max(minFontPx, Math.floor(baseSizes[i] * scale));
        el.style.fontSize = `${sz}px`;
        el.setAttribute('data-ppt-font-size', String(sz));
      });
      void wrapper.offsetHeight; // force reflow
    };

    // Start at 95 % and step down 5 % each iteration (max ~18 steps to 5 %)
    let scale = 0.95;
    while (scale >= minScale) {
      applyScale(scale);
      if (!isOverflowing()) break;
      scale = Math.round((scale - 0.05) * 100) / 100;
    }
  }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function parsePx(style: string, prop: string): number {
  const m = style.match(new RegExp(`(?:^|;|\\s)${prop}:\\s*(-?[\\d.]+)px`));
  return m ? parseFloat(m[1]) : 0;
}

function normalizeColor(raw: string): string {
  if (!raw || raw === 'inherit' || raw === 'transparent') return 'FFFFFF';
  const hex = raw.replace('#', '').trim();
  return (hex.length === 3
    ? hex.split('').map(c => c + c).join('')
    : hex
  ).slice(0, 6).padEnd(6, 'F').toUpperCase();
}

function forceVisible(slideEl: HTMLElement) {
  slideEl.querySelectorAll<HTMLElement>('[id^="layer-"]').forEach(el => {
    el.style.cssText += ';opacity:1!important;transform:none!important;visibility:visible!important;animation:none!important;';
  });
  slideEl.getAnimations().forEach(a => {
    try { (a as Animation & { commitStyles(): void }).commitStyles(); } catch { /* */ }
    try { a.cancel(); } catch { /* */ }
  });
}

// ── Animation info ────────────────────────────────────────────────────────────

interface AnimInfo {
  shapeId: number;
  animation: string;  // 'fade-in', 'fly-in-bottom', etc.
  clickOrder: number; // 0 = on entry, 1+ = click reveal
  durationMs: number;
}

// ── Chart initialization ──────────────────────────────────────────────────────

function initCharts(container: HTMLElement): Chart[] {
  const instances: Chart[] = [];
  for (const canvas of Array.from(container.querySelectorAll<HTMLCanvasElement>('canvas[data-chart]'))) {
    try {
      // Destroy any existing Chart instance on this canvas
      const existing = Chart.getChart(canvas);
      if (existing) existing.destroy();
      const config = JSON.parse(canvas.getAttribute('data-chart')!);
      const parent = canvas.parentElement;
      canvas.width  = parent?.offsetWidth  || parseInt(canvas.style.width)  || 400;
      canvas.height = parent?.offsetHeight || parseInt(canvas.style.height) || 300;
      instances.push(new Chart(canvas, config));
    } catch { /* malformed config */ }
  }
  return instances;
}

// ── Image / Chart layer extraction ───────────────────────────────────────────

interface ImageLayer {
  x: number; y: number; w: number; h: number;
  data: string;
  anim: AnimInfo | null;
}

// ── Layer capture helpers ─────────────────────────────────────────────────────

/**
 * Resolve SVG currentColor to explicit hex so html2canvas (which serializes
 * SVG to a data-URL before drawing) picks up the right stroke/fill color.
 */
function bakeLucideColors(el: HTMLElement): void {
  el.querySelectorAll<SVGElement>('svg[data-lucide]').forEach(svg => {
    const color = window.getComputedStyle(svg as unknown as Element).color;
    if (!color) return;
    svg.querySelectorAll<SVGElement>('*').forEach(child => {
      if (child.getAttribute('stroke') === 'currentColor') child.setAttribute('stroke', color);
      if (child.getAttribute('fill')   === 'currentColor') child.setAttribute('fill',   color);
    });
  });
}

/** Capture a positioned layer element as a PNG via html2canvas. */
async function captureLayerAsPng(
  el: HTMLElement,
  slideRect: DOMRect,
): Promise<ImageLayer | null> {
  const style = el.getAttribute('style') || '';
  const x = parsePx(style, 'left');
  const y = parsePx(style, 'top');
  const w = parsePx(style, 'width') || SLIDE_W_PX;
  const h = parsePx(style, 'height') || 60;
  if (!w || !h) return null;

  const prevVis = el.style.visibility;
  const prevOpa = el.style.opacity;
  el.style.visibility = 'visible';
  el.style.opacity    = '1';

  let data = '';
  try {
    const canvas = await html2canvas(el, {
      scale: 2,
      useCORS: true,
      allowTaint: true,
      logging: false,
      backgroundColor: null,
      width:  Math.ceil(w),
      height: Math.ceil(h),
      windowWidth:  document.documentElement.scrollWidth,
      windowHeight: document.documentElement.scrollHeight,
    });
    data = canvas.toDataURL('image/png');
  } catch { /* fall through */ } finally {
    el.style.visibility = prevVis;
    el.style.opacity    = prevOpa;
  }

  if (!data || data.length < 200) return null;

  const animation  = el.getAttribute('data-ppt-animation') || 'fade-in';
  const clickOrder = parseInt(el.getAttribute('data-click')    || '0');
  const durationMs = parseInt((el.getAttribute('data-duration') || '600').replace(/[^0-9]/g, ''));

  return { x, y, w, h, data, anim: { shapeId: 0, animation, clickOrder, durationMs } };
}


function extractImageLayer(el: HTMLElement, shapeId: number): ImageLayer | null {
  const style = el.getAttribute('style') || '';
  const x = parsePx(style, 'left');
  const y = parsePx(style, 'top');
  const w = parsePx(style, 'width');
  const h = parsePx(style, 'height');
  if (!w || !h) return null;

  let data = '';

  // Chart layer — export canvas as PNG
  if (/^layer-chart-/.test(el.id)) {
    const canvas = el.querySelector<HTMLCanvasElement>('canvas');
    if (canvas) data = canvas.toDataURL('image/png');
  } else if (el.tagName === 'IMG') {
    const src = (el as HTMLImageElement).src;
    if (src?.startsWith('data:')) data = src;
  } else {
    const bgMatch = style.match(/background-image:\s*url\(['"]?(data:[^'")\s]+)['"]?\)/);
    if (bgMatch) data = bgMatch[1];
  }
  if (!data) return null;

  const animation = el.getAttribute('data-ppt-animation') || 'fade-in';
  const clickOrder = parseInt(el.getAttribute('data-click') || '0');
  const durationMs = parseInt(el.getAttribute('data-duration') || '600');

  return {
    x, y, w, h, data,
    anim: { shapeId, animation, clickOrder, durationMs },
  };
}

// ── Text layer extraction ─────────────────────────────────────────────────────

type PptxAlign = 'left' | 'center' | 'right';

interface TextRun {
  text: string;
  options: {
    fontSize: number;
    bold: boolean;
    italic: boolean;
    color: string;
    fontFace: string;
    align: PptxAlign;
    breakLine: boolean;
  };
}

interface TextLayer {
  x: number; y: number; w: number; h: number;
  runs: TextRun[];
  anim: AnimInfo | null;
}

function extractTextLayer(el: HTMLElement, shapeId: number): TextLayer | null {
  const style = el.getAttribute('style') || '';
  const x = parsePx(style, 'left');
  const y = parsePx(style, 'top');
  const w = parsePx(style, 'width') || SLIDE_W_PX;
  const h = parsePx(style, 'height') || 60;

  const animation = el.getAttribute('data-ppt-animation') || 'fade-in';
  const clickOrder = parseInt(el.getAttribute('data-click') || '0');
  const durationMs = parseInt(el.getAttribute('data-duration') || '600');
  const anim: AnimInfo = { shapeId, animation, clickOrder, durationMs };

  const textEls = Array.from(el.querySelectorAll<HTMLElement>('[data-ppt-font-size]'));

  if (textEls.length === 0) {
    const text = el.textContent?.trim();
    if (!text) return null;
    return {
      x, y, w, h, anim,
      runs: [{ text, options: { fontSize: 20, bold: false, italic: false, color: 'FFFFFF', fontFace: 'Calibri', align: 'left', breakLine: false } }],
    };
  }

  const runs: TextRun[] = [];
  for (let i = 0; i < textEls.length; i++) {
    const te = textEls[i];
    let text = te.textContent?.trim() || '';
    if (!text) continue;
    if (te.tagName === 'LI') text = `• ${text}`;

    const fontSize  = parseInt(te.getAttribute('data-ppt-font-size') || '20') || 20;
    const bold      = te.getAttribute('data-ppt-bold') === 'true';
    const color     = normalizeColor(te.getAttribute('data-ppt-color') || '#ffffff');
    const fontFace  = te.getAttribute('data-ppt-font') || 'Calibri';
    const alignRaw  = te.getAttribute('data-ppt-align') || 'left';
    const align     = (['left', 'center', 'right'].includes(alignRaw) ? alignRaw : 'left') as PptxAlign;
    const italic    = window.getComputedStyle(te).fontStyle === 'italic';

    runs.push({
      text,
      options: { fontSize, bold, italic, color, fontFace, align, breakLine: i < textEls.length - 1 },
    });
  }

  return runs.length > 0 ? { x, y, w, h, runs, anim } : null;
}

// ── Per-slide render ──────────────────────────────────────────────────────────

async function renderSlideForExport(html: string): Promise<{ bgDataUrl: string; imageLayers: ImageLayer[]; textLayers: TextLayer[]; faCaptureLayers: ImageLayer[]; transition: string }> {
  const wrap = document.createElement('div');
  wrap.style.cssText = `position:fixed;left:-9999px;top:0;width:${SLIDE_W_PX}px;height:${SLIDE_H_PX}px;overflow:hidden;`;
  wrap.innerHTML = html;
  const slideEl = wrap.querySelector<HTMLElement>('.ppt-slide') ?? wrap;
  if (slideEl !== wrap) slideEl.classList.add('ppt-preview');
  document.body.appendChild(wrap);

  try {
    forceVisible(slideEl);

    const textLayerEls  = Array.from(slideEl.querySelectorAll<HTMLElement>('[id^="layer-text-"]'));
    const imageLayerEls = Array.from(slideEl.querySelectorAll<HTMLElement>('[id^="layer-image-"], [id^="layer-chart-"]'));

    await inlineImages(slideEl);
    bakeStyles(slideEl);

    // Initialize Chart.js charts so they render before we capture
    const chartInstances = initCharts(slideEl);
    // Give canvas a tick to paint
    await new Promise(r => requestAnimationFrame(r));
    try { await document.fonts.ready; } catch { /* ignore */ }

    // Convert <i data-lucide="..."> → inline <svg> then bake currentColor
    createIcons({ icons: lucideIcons, root: slideEl });
    bakeLucideColors(slideEl);

    // Auto-fit: shrink any overflowing text layers before capture / text extraction
    fitTextToLayers(slideEl);

    // Shape IDs: bg=2, reg images start at 3. IDs assigned after filtering nulls.
    const imageStartId = 3;

    // Extract image/chart layers (placeholder ID 0, fixed below)
    const imageLayers = imageLayerEls
      .map(el => extractImageLayer(el, 0))
      .filter((l): l is ImageLayer => l !== null);
    imageLayers.forEach((l, i) => { if (l.anim) l.anim.shapeId = imageStartId + i; });

    chartInstances.forEach(c => c.destroy());

    const textStartId = imageStartId + imageLayers.length;

    // Hide ALL layers, re-show only bg/deco/overlay for background render
    const allLayerEls = Array.from(slideEl.querySelectorAll<HTMLElement>('[id^="layer-"]'));
    allLayerEls.forEach(el => { el.style.visibility = 'hidden'; el.style.opacity = '0'; });
    allLayerEls
      .filter(el => /^layer-(bg|deco|overlay)-/.test(el.id))
      .forEach(el => { el.style.visibility = 'visible'; el.style.opacity = '1'; });

    const bgCanvas = await html2canvas(slideEl, { ...CANVAS_OPTS, width: SLIDE_W_PX, height: SLIDE_H_PX });
    const bgDataUrl = bgCanvas.toDataURL('image/png');

    // Re-show text layers to parse/capture
    textLayerEls.forEach(el => { el.style.visibility = 'visible'; el.style.opacity = '1'; });

    const slideRect = slideEl.getBoundingClientRect();

    // Split text layers: icon layers → capture as PNG; others → extract as text
    const hasIcon = (el: HTMLElement) => el.querySelector('svg[data-lucide]') !== null;

    const textLayers:      TextLayer[]  = [];
    const faCaptureLayers: ImageLayer[] = [];

    for (const el of textLayerEls) {
      if (hasIcon(el)) {
        const layer = await captureLayerAsPng(el, slideRect);
        if (layer) faCaptureLayers.push(layer);
      } else {
        const layer = extractTextLayer(el, 0);
        if (layer) textLayers.push(layer);
      }
    }

    // Shape ID sequence: bg=2 → imageLayers → textLayers → iconCaptureLayers
    const faCaptureStartId = textStartId + textLayers.length;
    textLayers.forEach((l, i)      => { if (l.anim) l.anim.shapeId = textStartId      + i; });
    faCaptureLayers.forEach((l, i) => { if (l.anim) l.anim.shapeId = faCaptureStartId + i; });

    const transition = slideEl.getAttribute('data-transition') ?? 'none';
    return { bgDataUrl, imageLayers, textLayers, faCaptureLayers, transition };
  } finally {
    document.body.removeChild(wrap);
  }
}

// ── Animation XML generation ──────────────────────────────────────────────────

// Preset IDs → [presetID, presetSubtype]
// presetID + presetSubtype values from real PowerPoint XML (fly directions use subtype bitmask)
// Fly In subtypes: right=1, left=2, bottom=4 (default), top=8
const ANIM_PRESET: Record<string, [number, number]> = {
  'appear':        [1,  0],
  'fade-in':       [10, 0],
  'fly-in-bottom': [26, 4],
  'fly-in-top':    [26, 8],
  'fly-in-left':   [26, 2],
  'fly-in-right':  [26, 1],
  'zoom-in':       [26, 4],  // use fly+fade as zoom fallback (true zoom=62 needs verification)
  'float-in':      [10, 0],
  'wipe-left':     [57, 0],
  'bounce-in':     [10, 0],
  'split':         [10, 0],
  'swivel':        [10, 0],
};

// tgt() helper: <p:tgtEl><p:spTgt spid="N"/></p:tgtEl>
function tgt(spid: number) {
  return `<p:tgtEl><p:spTgt spid="${spid}"/></p:tgtEl>`;
}

// Visibility set — always first child of every effect
// MUST have <p:stCondLst> inside <p:cTn> (omitting this breaks PowerPoint)
function visSet(spid: number, id: () => number): string {
  return `<p:set><p:cBhvr><p:cTn id="${id()}" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn>${tgt(spid)}<p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="visible"/></p:to></p:set>`;
}

// Fade opacity: 0→1 over `dur` ms
function fadeAnim(spid: number, dur: number, id: () => number): string {
  return `<p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn id="${id()}" dur="${dur}"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn>${tgt(spid)}</p:cBhvr></p:animEffect>`;
}

// Fly: wipe reveal + linear translate from outside slide
// filter: wipe(down)=from-bottom, wipe(up)=from-top, wipe(right)=from-left, wipe(left)=from-right
function flyAnim(spid: number, dur: number, fromX: string, fromY: string, wipeFilter: string, id: () => number): string {
  const wipeId = id();
  const xId    = id();
  const yId    = id();
  // Brief wipe reveal (same ratio PowerPoint uses: ~20% of total dur)
  const wipeDur = Math.round(dur * 0.2);
  const parts = [
    `<p:animEffect transition="in" filter="${wipeFilter}"><p:cBhvr><p:cTn id="${wipeId}" dur="${wipeDur}"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn>${tgt(spid)}</p:cBhvr></p:animEffect>`,
  ];
  if (fromX !== '#ppt_x') {
    parts.push(`<p:anim calcmode="lin" valueType="num"><p:cBhvr><p:cTn id="${xId}" dur="${dur}" tmFilter="0,0;0.14,0.36;0.43,0.73;0.71,0.91;1.0,1.0"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn>${tgt(spid)}<p:attrNameLst><p:attrName>ppt_x</p:attrName></p:attrNameLst></p:cBhvr><p:tavLst><p:tav tm="0"><p:val><p:strVal val="${fromX}"/></p:val></p:tav><p:tav tm="100000"><p:val><p:strVal val="#ppt_x"/></p:val></p:tav></p:tavLst></p:anim>`);
  }
  if (fromY !== '#ppt_y') {
    parts.push(`<p:anim calcmode="lin" valueType="num"><p:cBhvr><p:cTn id="${yId}" dur="${dur}" tmFilter="0,0;0.14,0.36;0.43,0.73;0.71,0.91;1.0,1.0"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn>${tgt(spid)}<p:attrNameLst><p:attrName>ppt_y</p:attrName></p:attrNameLst></p:cBhvr><p:tavLst><p:tav tm="0"><p:val><p:strVal val="${fromY}"/></p:val></p:tav><p:tav tm="100000"><p:val><p:strVal val="#ppt_y"/></p:val></p:tav></p:tavLst></p:anim>`);
  }
  return parts.join('');
}

function buildEffectXml(anim: string, spid: number, dur: number, id: () => number): string {
  const vis = visSet(spid, id);
  switch (anim) {
    case 'appear':
      return vis; // instant reveal, no animation primitive
    case 'fade-in':
    case 'float-in':
    case 'bounce-in':
    case 'swivel':
    case 'split':
      return vis + fadeAnim(spid, dur, id);
    case 'fly-in-bottom':
      return vis + flyAnim(spid, dur, '#ppt_x', '#ppt_y+1', 'wipe(down)', id);
    case 'fly-in-top':
      return vis + flyAnim(spid, dur, '#ppt_x', '#ppt_y-1', 'wipe(up)', id);
    case 'fly-in-left':
      return vis + flyAnim(spid, dur, '#ppt_x-1', '#ppt_y', 'wipe(right)', id);
    case 'fly-in-right':
      return vis + flyAnim(spid, dur, '#ppt_x+1', '#ppt_y', 'wipe(left)', id);
    case 'zoom-in':
      // fade + scale: start at 30% size
      return vis
        + fadeAnim(spid, dur, id)
        + `<p:animScale><p:cBhvr><p:cTn id="${id()}" dur="${dur}"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn>${tgt(spid)}</p:cBhvr><p:from x="30000" y="30000"/><p:to x="100000" y="100000"/></p:animScale>`;
    case 'wipe-left':
      return vis + `<p:animEffect transition="in" filter="wipe(left)"><p:cBhvr><p:cTn id="${id()}" dur="${dur}"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn>${tgt(spid)}</p:cBhvr></p:animEffect>`;
    default:
      return vis + fadeAnim(spid, dur, id);
  }
}

function buildTimingXml(allAnims: AnimInfo[]): string {
  const animated = allAnims.filter(a => a.animation !== 'appear' || a.clickOrder > 0);
  if (animated.length === 0) return '';

  // IDs 1 and 2 are reserved for root and mainSeq — start counter at 2
  let idCounter = 2;
  const nextId = () => ++idCounter;

  const entryAnims = allAnims.filter(a => a.clickOrder === 0);
  const clickAnims = allAnims.filter(a => a.clickOrder > 0);

  // Group click anims by click order, preserving order
  const byClick = new Map<number, AnimInfo[]>();
  for (const a of clickAnims) {
    if (!byClick.has(a.clickOrder)) byClick.set(a.clickOrder, []);
    byClick.get(a.clickOrder)!.push(a);
  }
  const sortedClicks = [...byClick.keys()].sort((a, b) => a - b);

  let seqChildren = '';

  // ── Group 0: auto-play on slide load (delay="0", not indefinite) ──
  if (entryAnims.length > 0) {
    const outer = nextId();
    const inner = nextId();
    let effects = '';
    let grpId = 0;
    for (const a of entryAnims) {
      const [presetId, presetSubtype] = ANIM_PRESET[a.animation] ?? [10, 0];
      const ctnId = nextId();
      const effectXml = buildEffectXml(a.animation, a.shapeId, a.durationMs, nextId);
      // First effect is clickEffect (plays immediately), rest are withEffect
      const nodeType = effects === '' ? 'clickEffect' : 'withEffect';
      effects += `<p:par><p:cTn id="${ctnId}" presetID="${presetId}" presetClass="entr" presetSubtype="${presetSubtype}" fill="hold" grpId="${grpId}" nodeType="${nodeType}"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>${effectXml}</p:childTnLst></p:cTn></p:par>`;
    }
    seqChildren += `<p:par><p:cTn id="${outer}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="${inner}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>${effects}</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>`;
  }

  // ── Click groups (delay="indefinite") ────────────────────────────────
  for (const click of sortedClicks) {
    const group = byClick.get(click)!;
    const outer  = nextId();
    const inner  = nextId();
    const grpId  = click; // use click index as group id
    let effects  = '';

    for (const a of group) {
      const [presetId, presetSubtype] = ANIM_PRESET[a.animation] ?? [10, 0];
      const ctnId     = nextId();
      const effectXml = buildEffectXml(a.animation, a.shapeId, a.durationMs, nextId);
      const nodeType  = effects === '' ? 'clickEffect' : 'withEffect';
      effects += `<p:par><p:cTn id="${ctnId}" presetID="${presetId}" presetClass="entr" presetSubtype="${presetSubtype}" fill="hold" grpId="${grpId}" nodeType="${nodeType}"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>${effectXml}</p:childTnLst></p:cTn></p:par>`;
    }

    // delay="indefinite" = wait for click before this group plays
    seqChildren += `<p:par><p:cTn id="${outer}" fill="hold"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst><p:par><p:cTn id="${inner}" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>${effects}</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>`;
  }

  // bldLst: one entry per animated shape, no extra attributes
  const bldEntries = animated
    .map(a => `<p:bldP spid="${a.shapeId}" grpId="${a.clickOrder}"/>`)
    .join('');

  // prevCondLst/nextCondLst: exact format from PowerPoint
  const prevCond = `<p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst>`;
  const nextCond = `<p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst>`;

  // Root id=1, mainSeq id=2
  return `<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst><p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>${seqChildren}</p:childTnLst></p:cTn>${prevCond}${nextCond}</p:seq></p:childTnLst></p:cTn></p:par></p:tnLst><p:bldLst>${bldEntries}</p:bldLst></p:timing>`;
}

// ── Slide transition XML builder ───────────────────────────────────────────────

function buildTransitionXml(transition: string): string {
  switch (transition) {
    case 'fade':    return '<p:transition spd="med"><p:fade/></p:transition>';
    case 'push':    return '<p:transition><p:push/></p:transition>';
    case 'wipe':    return '<p:transition spd="med"><p:wipe dir="r"/></p:transition>';
    case 'cover':   return '<p:transition spd="med"><p:cover dir="r"/></p:transition>';
    default:        return '';
  }
}

// ── Inject timing + transitions into PPTX blob ────────────────────────────────

async function injectAnimations(
  pptxBuffer: ArrayBuffer,
  slideTimings: string[],      // one <p:timing> XML string per slide (empty = no anim)
  slideTransitions: string[],  // one transition name per slide (empty|'none' = skip)
): Promise<Uint8Array> {
  const zip = await JSZip.loadAsync(pptxBuffer);

  for (let i = 0; i < Math.max(slideTimings.length, slideTransitions.length); i++) {
    const timing     = slideTimings[i]     ?? '';
    const transition = slideTransitions[i] ?? 'none';

    if (!timing && (!transition || transition === 'none')) continue;

    const slideFile = zip.file(`ppt/slides/slide${i + 1}.xml`);
    if (!slideFile) continue;

    let xml = await slideFile.async('string');

    // Build the XML to inject after </p:clrMapOvr> (correct OOXML position).
    // Order: <p:transition> then <p:timing> (per CT_Slide schema).
    const transitionXml = buildTransitionXml(transition);
    const inject = transitionXml + timing;

    if (inject) {
      // Insert before </p:sld> — this is always safe regardless of whether
      // <p:clrMapOvr> is present or not.
      xml = xml.replace('</p:sld>', `${inject}</p:sld>`);
    }

    zip.file(`ppt/slides/slide${i + 1}.xml`, xml);
  }

  return zip.generateAsync({ type: 'uint8array', compression: 'DEFLATE' });
}

// ── Export ────────────────────────────────────────────────────────────────────

export async function exportToPptx(
  deckData: DeckData,
  onProgress?: (current: number, total: number) => void,
): Promise<void> {
  // Pick save path first
  const savePath = await save({
    defaultPath: `${sanitizeFilename(deckData.title || 'presentation')}.pptx`,
    filters: [{ name: 'PowerPoint', extensions: ['pptx'] }],
  });
  if (!savePath) return; // user cancelled

  const pptx = new PptxGenJS();
  pptx.layout = 'LAYOUT_WIDE';

  const slideTimings: string[] = [];
  const slideTransitions: string[] = [];

  for (const [i, slideData] of deckData.slides.entries()) {
    const { bgDataUrl, imageLayers, textLayers, faCaptureLayers, transition } =
      await renderSlideForExport(slideData.html);

    const slide = pptx.addSlide();

    // Z-order: bg → reg images → text boxes → FA-captured PNGs

    // 1. Background
    slide.addImage({ data: bgDataUrl, x: 0, y: 0, w: PPTX_W_IN, h: PPTX_H_IN });

    // 2. Regular image / chart layers
    for (const img of imageLayers) {
      slide.addImage({
        data: img.data,
        x: pxXtoIn(img.x), y: pxYtoIn(img.y),
        w: pxXtoIn(img.w), h: pxYtoIn(img.h),
      });
    }

    // 3. Text boxes (editable)
    for (const layer of textLayers) {
      slide.addText(layer.runs as Parameters<typeof slide.addText>[0], {
        x: pxXtoIn(layer.x), y: pxYtoIn(layer.y),
        w: pxXtoIn(layer.w), h: pxYtoIn(layer.h),
        valign: 'middle', wrap: true, margin: 0,
      });
    }

    // 4. FA-containing layers as PNG
    for (const layer of faCaptureLayers) {
      slide.addImage({
        data: layer.data,
        x: pxXtoIn(layer.x), y: pxYtoIn(layer.y),
        w: pxXtoIn(layer.w), h: pxYtoIn(layer.h),
      });
    }

    const allAnims: AnimInfo[] = [
      ...imageLayers.filter(l => l.anim).map(l => l.anim!),
      ...textLayers.filter(l => l.anim).map(l => l.anim!),
      ...faCaptureLayers.filter(l => l.anim).map(l => l.anim!),
    ];
    slideTimings.push(buildTimingXml(allAnims));
    slideTransitions.push(transition);

    onProgress?.(i + 1, deckData.slides.length);
  }

  // Generate PPTX as ArrayBuffer, inject animations + transitions, save
  const buffer = await pptx.write({ outputType: 'arraybuffer' }) as ArrayBuffer;
  const patched = await injectAnimations(buffer, slideTimings, slideTransitions);
  await writeFile(savePath, patched);
}

function sanitizeFilename(name: string): string {
  return name.replace(/[<>:"/\\|?*\x00-\x1f]/g, '_').slice(0, 200);
}
