import { Download, Monitor, X } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { SLIDE_HEIGHT, SLIDE_WIDTH } from '../constants';
import { cn } from '../lib/utils';
import type { DeckData } from '../types';

interface SlidePreviewProps {
  deckData: DeckData | null;
  isOpen: boolean;
  onClose: () => void;
  onExport: () => void;
  isExporting?: boolean;
  exportProgress?: number;
  imageStatus?: string | null;
}

const THUMB_W = 84;
const THUMB_H = 47;

export function SlidePreview({ deckData, isOpen, onClose, onExport, isExporting = false, exportProgress = 0, imageStatus }: SlidePreviewProps) {
  const [currentSlide, setCurrentSlide] = useState(0);
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const slides = deckData?.slides || [];

  // Track new slides for animation
  const seenIds = useRef(new Set<string>());
  const newSlideIds = useRef(new Set<string>());

  useEffect(() => {
    if (!deckData || slides.length === 0) {
      seenIds.current.clear();
      newSlideIds.current.clear();
      setCurrentSlide(0);
      return;
    }
    const fresh = slides.filter(s => !seenIds.current.has(s.id));
    if (fresh.length > 0) {
      fresh.forEach(s => {
        seenIds.current.add(s.id);
        newSlideIds.current.add(s.id);
      });
      setCurrentSlide(slides.length - 1);
      const t = setTimeout(() => fresh.forEach(s => newSlideIds.current.delete(s.id)), 400);
      return () => clearTimeout(t);
    }
  }, [deckData, slides]);

  // Receive slide-change events from iframe
  useEffect(() => {
    function onMessage(e: MessageEvent) {
      if (e.data?.type === 'slideChange') {
        setCurrentSlide(e.data.index ?? 0);
      }
    }
    window.addEventListener('message', onMessage);
    return () => window.removeEventListener('message', onMessage);
  }, []);

  // Forward AI image-ready events into iframe
  useEffect(() => {
    function onImageReady(e: Event) {
      const { prompt, url } = (e as CustomEvent<{ prompt: string; url: string }>).detail;
      iframeRef.current?.contentWindow?.postMessage({ type: 'updateImage', prompt, url }, '*');
    }
    window.addEventListener('deck-image-ready', onImageReady);
    return () => window.removeEventListener('deck-image-ready', onImageReady);
  }, []);

  // Tell iframe to go to a specific slide
  const gotoSlide = useCallback((i: number) => {
    iframeRef.current?.contentWindow?.postMessage({ type: 'goto', index: i }, '*');
    setCurrentSlide(i);
  }, []);

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'ArrowRight' || e.key === ' ') {
      iframeRef.current?.contentWindow?.postMessage({ type: 'advance' }, '*');
    }
    if (e.key === 'ArrowLeft') {
      iframeRef.current?.contentWindow?.postMessage({ type: 'retreat' }, '*');
    }
  }, []);

  if (!isOpen) return null;

  const thumbScale = THUMB_W / SLIDE_WIDTH;

  return (
    <div
      className="flex flex-col h-full w-full bg-surface border-l border-line"
      tabIndex={0}
      onKeyDown={handleKeyDown}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 h-10 border-b border-line shrink-0">
        <div className="flex items-center gap-2.5">
          <span className="text-[14px] font-medium text-fg-3">Preview</span>
          {slides.length > 0 && (
            <span className="text-[14px] text-fg-4 bg-input border border-line px-1.5 py-px rounded-md">
              {currentSlide + 1} / {slides.length}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {slides.length > 0 && (
            <button
              onClick={onExport}
              disabled={isExporting}
              className="relative flex items-center gap-1.5 px-3 py-1.5 bg-cta hover:bg-cta-hv text-cta-fg text-[14px] font-medium rounded-lg transition-colors disabled:cursor-not-allowed overflow-hidden"
              style={{ minWidth: isExporting ? 120 : undefined }}
            >
              {/* Progress fill */}
              {isExporting && (
                <span
                  className="absolute inset-0 bg-white/15 origin-left transition-all duration-300 ease-out"
                  style={{ transform: `scaleX(${exportProgress / 100})` }}
                />
              )}
              {/* Shimmer sweep */}
              {isExporting && (
                <span className="absolute inset-0 bg-gradient-to-r from-transparent via-white/10 to-transparent animate-[shimmer_1.2s_ease-in-out_infinite]" />
              )}
              <span className="relative flex items-center gap-1.5">
                {isExporting ? (
                  <>
                    <svg className="animate-spin shrink-0" width={11} height={11} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.5}>
                      <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/>
                    </svg>
                    <span className="tabular-nums">
                      {Math.round(exportProgress / 100 * slides.length)}/{slides.length}
                    </span>
                  </>
                ) : (
                  <>
                    <Download size={11} />
                    Export PPTX
                  </>
                )}
              </span>
            </button>
          )}
          <button onClick={onClose} className="p-2.5 rounded-md text-fg-5 hover:text-fg-3 transition-colors">
            <X size={15} />
          </button>
        </div>
      </div>

      {/* Thumbnail strip */}
      {slides.length > 0 && (
        <div className="flex gap-1.5 px-3 py-2 overflow-x-auto border-b border-line shrink-0">
          {slides.map((s, i) => (
            <button
              key={s.id}
              onClick={() => gotoSlide(i)}
              className={cn(
                'shrink-0 overflow-hidden rounded border-2 transition-all',
                newSlideIds.current.has(s.id) && 'slide-thumb-enter',
                i === currentSlide ? 'border-cta' : 'border-line hover:border-line-hi'
              )}
              style={{ width: THUMB_W, height: THUMB_H }}
            >
              {/* Static thumbnail — dangerouslySetInnerHTML, no scripts, ppt-preview shows all elements */}
              <div
                className="ppt-preview"
                style={{
                  width: SLIDE_WIDTH,
                  height: SLIDE_HEIGHT,
                  transform: `scale(${thumbScale})`,
                  transformOrigin: 'top left',
                  pointerEvents: 'none',
                }}
                dangerouslySetInnerHTML={{ __html: s.html }}
              />
            </button>
          ))}
        </div>
      )}

      {/* Main viewer — single iframe with master HTML */}
      <div className="flex-1 relative overflow-hidden bg-bg">
        {!deckData || slides.length === 0 ? (
          <div className="h-full flex flex-col items-center justify-center gap-2">
            <Monitor size={32} className="text-fg-4" strokeWidth={1.5} />
            <p className="text-[14px] text-fg-3">No slides yet</p>
          </div>
        ) : deckData.masterHtml ? (
          <iframe
            ref={iframeRef}
            srcDoc={deckData.masterHtml}
            sandbox="allow-scripts"
            style={{ width: '100%', height: '100%', border: 'none', background: '#111' }}
            title="Presentation"
          />
        ) : (
          // Fallback: old dangerouslySetInnerHTML approach for sessions without masterHtml
          <div className="h-full flex items-center justify-center">
            <div
              className="ppt-preview"
              style={{ width: SLIDE_WIDTH, height: SLIDE_HEIGHT, transform: 'scale(0.58)', transformOrigin: 'center' }}
              dangerouslySetInnerHTML={{ __html: slides[currentSlide]?.html || '' }}
            />
          </div>
        )}
      </div>

      {/* Status bar */}
      <div className="px-3 h-7 border-t border-line shrink-0 flex items-center justify-between">
        <span className="text-[12px] text-fg-3">
          {imageStatus
            ? <span className={imageStatus.startsWith('Image failed') || imageStatus.startsWith('No image') ? 'text-red-400' : 'text-yellow-400'}>{imageStatus}</span>
            : (slides[currentSlide] ? slides[currentSlide].type : '—')
          }
        </span>
        {deckData?.theme && (
          <span className="text-[12px] text-fg-3">{deckData.theme.style}</span>
        )}
      </div>
    </div>
  );
}
