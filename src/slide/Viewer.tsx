/**
 * Deckr Presentation Viewer
 * PowerPoint-like slide viewer with click-to-reveal animations and transitions.
 */
import { useCallback, useEffect, useRef, useState } from 'react';
import './animations.css';
import type { DeckData, Slide } from '../types';

interface ViewerProps {
  deckData: DeckData;
  onClose?: () => void;
}

interface AnimState {
  slideIndex: number;
  clickStep: number;     // Current step in the sequence
  clickSequence: number[]; // Array of unique data-click values > 0, sorted
  animating: boolean;
}

export function PresentationViewer({ deckData, onClose }: ViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const slideRefs = useRef<Map<number, HTMLDivElement>>(new Map());
  const [state, setState] = useState<AnimState>({
    slideIndex: 0,
    clickStep: 0,
    clickSequence: [],
    animating: false,
  });
  const [scale, setScale] = useState(1);
  const [direction, setDirection] = useState<'forward' | 'back'>('forward');
  const [prevIndex, setPrevIndex] = useState<number | null>(null);
  const [isTransitioning, setIsTransitioning] = useState(false);

  const slides = deckData.slides;
  const currentSlide = slides[state.slideIndex];

  // ── Compute scale to fill the screen ────────────────────────────────────
  useEffect(() => {
    const update = () => {
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      const s = Math.min(vw / 960, vh / 540);
      setScale(s);
    };
    update();
    window.addEventListener('resize', update);
    return () => window.removeEventListener('resize', update);
  }, []);

  // ── Get sequence of animated clicks on a slide ──────────────────────────
  const getClickSequenceForSlide = useCallback((slideIndex: number): number[] => {
    const el = slideRefs.current.get(slideIndex);
    if (!el) return [];
    const clicks = new Set<number>();
    el.querySelectorAll('[data-click]').forEach(e => {
      const click = parseInt((e as HTMLElement).dataset.click || '0', 10);
      if (click > 0) clicks.add(click);
    });
    return Array.from(clicks).sort((a, b) => a - b);
  }, []);

  // ── Web Animations API helper ─────────────────────────────────────────────
  const animateElement = useCallback((el: HTMLElement, anim: string, dur: number) => {
    el.getAnimations().forEach(a => a.cancel());
    el.style.visibility = 'visible';
    type KF = Keyframe[];
    type KO = KeyframeAnimationOptions;
    const defs: Record<string, [KF, KO]> = {
      'appear':        [[{opacity:0},{opacity:1}], {duration:Math.min(dur,100), fill:'forwards'}],
      'fade-in':       [[{opacity:0},{opacity:1}], {duration:dur, easing:'ease', fill:'forwards'}],
      'fly-in-bottom': [[{opacity:0,transform:'translateY(110%)'},{opacity:1,transform:'translateY(0)'}], {duration:dur, easing:'cubic-bezier(.25,.46,.45,.94)', fill:'forwards'}],
      'fly-in-top':    [[{opacity:0,transform:'translateY(-110%)'},{opacity:1,transform:'translateY(0)'}], {duration:dur, easing:'cubic-bezier(.25,.46,.45,.94)', fill:'forwards'}],
      'fly-in-left':   [[{opacity:0,transform:'translateX(-110%)'},{opacity:1,transform:'translateX(0)'}], {duration:dur, easing:'cubic-bezier(.25,.46,.45,.94)', fill:'forwards'}],
      'fly-in-right':  [[{opacity:0,transform:'translateX(110%)'},{opacity:1,transform:'translateX(0)'}], {duration:dur, easing:'cubic-bezier(.25,.46,.45,.94)', fill:'forwards'}],
      'zoom-in':       [[{opacity:0,transform:'scale(0.3)'},{opacity:1,transform:'scale(1)'}], {duration:dur, easing:'cubic-bezier(.175,.885,.32,1.275)', fill:'forwards'}],
      'bounce-in':     [[{opacity:0,transform:'scale(0.3)'},{opacity:1,transform:'scale(1.08)'},{opacity:1,transform:'scale(0.94)'},{opacity:1,transform:'scale(1)'}], {duration:dur, fill:'forwards'}],
      'float-in':      [[{opacity:0,transform:'translateY(36px)'},{opacity:1,transform:'translateY(0)'}], {duration:dur, easing:'cubic-bezier(.22,1,.36,1)', fill:'forwards'}],
      'wipe-left':     [[{clipPath:'inset(0 100% 0 0)'},{clipPath:'inset(0 0% 0 0)'}], {duration:dur, easing:'ease-out', fill:'forwards'}],
      'split':         [[{clipPath:'inset(50% 0)'},{clipPath:'inset(0% 0)'}], {duration:dur, easing:'ease-out', fill:'forwards'}],
      'swivel':        [[{opacity:0,transform:'perspective(800px) rotateY(-90deg)'},{opacity:1,transform:'perspective(800px) rotateY(0)'}], {duration:dur, easing:'cubic-bezier(.4,0,.2,1)', fill:'forwards'}],
    };
    const [kf, opts] = defs[anim] ?? defs['fade-in'];
    // wipe/split use clip-path, not opacity — make element opaque first
    if (anim === 'wipe-left' || anim === 'split') el.style.opacity = '1';
    el.animate(kf, opts);
  }, []);

  // ── Reveal elements for a given click number ─────────────────────────────
  const revealClick = useCallback((slideIndex: number, clickNum: number) => {
    const el = slideRefs.current.get(slideIndex);
    if (!el) return;
    const targets = el.querySelectorAll(`[data-click="${clickNum}"]`);
    targets.forEach(target => {
      const t = target as HTMLElement;
      const dur = parseInt(t.dataset.duration || '500', 10);
      const anim = t.dataset.pptAnimation || 'fade-in';
      t.classList.remove('ppt-hidden');
      t.style.opacity = '';
      animateElement(t, anim, dur);
    });
  }, [animateElement]);

  // ── Reveal click=0 elements immediately on slide enter ───────────────────
  const revealImmediateElements = useCallback((slideIndex: number) => {
    const el = slideRefs.current.get(slideIndex);
    if (!el) return;
    el.querySelectorAll('[data-click="0"]').forEach(target => {
      const t = target as HTMLElement;
      t.classList.remove('ppt-hidden');
      t.style.opacity = '';
      
      const dur = parseInt(t.dataset.duration || '500', 10);
      const anim = t.dataset.pptAnimation || 'fade-in';
      animateElement(t, anim, dur);
    });
  }, [animateElement]);

  // ── Reset all animations on a slide ─────────────────────────────────────
  const resetSlide = useCallback((slideIndex: number) => {
    const el = slideRefs.current.get(slideIndex);
    if (!el) return;
    el.querySelectorAll('[data-click]').forEach(target => {
      const t = target as HTMLElement;
      if (parseInt(t.dataset.click || '0', 10) > 0) {
        t.getAnimations().forEach(a => a.cancel());
        t.style.opacity = '0';
        t.style.visibility = 'hidden';
        t.style.transform = '';
        t.style.clipPath = '';
        t.classList.add('ppt-hidden');
      }
    });
  }, []);

  // ── Go to a slide with transition ────────────────────────────────────────
  const goToSlide = useCallback((targetIndex: number, dir: 'forward' | 'back') => {
    if (isTransitioning || targetIndex === state.slideIndex) return;
    if (targetIndex < 0 || targetIndex >= slides.length) return;

    setIsTransitioning(true);
    setDirection(dir);
    setPrevIndex(state.slideIndex);

    // Prepare target slide
    resetSlide(targetIndex);
    setTimeout(() => revealImmediateElements(targetIndex), 50);

    const clickSequence = getClickSequenceForSlide(targetIndex);
    setState({ slideIndex: targetIndex, clickStep: 0, clickSequence, animating: false });

    setTimeout(() => {
      setPrevIndex(null);
      setIsTransitioning(false);
    }, 500);
  }, [isTransitioning, state.slideIndex, slides.length, resetSlide, revealImmediateElements, getClickSequenceForSlide]);

  // ── Handle click: advance animation or slide ─────────────────────────────
  const handleClick = useCallback(() => {
    if (state.animating) return;

    if (state.clickStep < state.clickSequence.length) {
      // Reveal next group of elements
      const clickNum = state.clickSequence[state.clickStep];
      revealClick(state.slideIndex, clickNum);
      setState(s => ({ ...s, clickStep: s.clickStep + 1 }));
    } else {
      // All elements revealed — go to next slide
      if (state.slideIndex < slides.length - 1) {
        goToSlide(state.slideIndex + 1, 'forward');
      }
    }
  }, [state, revealClick, goToSlide, slides.length]);

  // ── Keyboard navigation ──────────────────────────────────────────────────
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowRight':
        case 'ArrowDown':
        case ' ':
        case 'PageDown':
          e.preventDefault();
          handleClick();
          break;
        case 'ArrowLeft':
        case 'ArrowUp':
        case 'PageUp':
          e.preventDefault();
          if (state.clickStep > 0) {
            // Reset to start of slide
            resetSlide(state.slideIndex);
            revealImmediateElements(state.slideIndex);
            setState(s => ({ ...s, clickStep: 0 }));
          } else if (state.slideIndex > 0) {
            goToSlide(state.slideIndex - 1, 'back');
          }
          break;
        case 'Escape':
          onClose?.();
          break;
        case 'Home':
          e.preventDefault();
          goToSlide(0, 'back');
          break;
        case 'End':
          e.preventDefault();
          goToSlide(slides.length - 1, 'forward');
          break;
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [handleClick, state, goToSlide, resetSlide, revealImmediateElements, onClose, slides.length]);

  // ── Handle slide HTML changes (e.g. streaming or edits) ────────────────
  useEffect(() => {
    if (!currentSlide) return;
    // Give React a tick to inject the HTML into the DOM
    const timer = setTimeout(() => {
      resetSlide(state.slideIndex);
      revealImmediateElements(state.slideIndex);
      const seq = getClickSequenceForSlide(state.slideIndex);
      
      setState(s => {
        // Restore elements that were already revealed if we are editing/updating mid-slide
        for (let i = 0; i < s.clickStep; i++) {
            if (i < seq.length) {
                // Instantly reveal without animation if possible, or just call revealClick
                const el = slideRefs.current.get(s.slideIndex);
                if (el) {
                    el.querySelectorAll(`[data-click="${seq[i]}"]`).forEach(target => {
                        const t = target as HTMLElement;
                        t.classList.remove('ppt-hidden');
                        t.style.opacity = '1';
                        t.style.visibility = 'visible';
                        t.style.transform = 'none';
                        t.style.clipPath = 'none';
                    });
                }
            }
        }
        return { ...s, clickSequence: seq };
      });
    }, 50);
    return () => clearTimeout(timer);
  }, [currentSlide?.html, state.slideIndex, resetSlide, revealImmediateElements, getClickSequenceForSlide]);

  // ── Get transition class ─────────────────────────────────────────────────
  const getTransitionClass = (slide: Slide, isEntering: boolean, isExiting: boolean) => {
    const t = (slide as any).transition || 'fade';
    if (isEntering) {
      if (direction === 'back') return 'tr-push-back-enter';
      return t === 'push' ? 'tr-push-enter' : t === 'wipe' ? 'tr-wipe-enter' : 'tr-fade-enter';
    }
    if (isExiting) {
      if (direction === 'back') return 'tr-push-back-exit';
      return t === 'push' ? 'tr-push-exit' : 'tr-fade-exit';
    }
    return '';
  };

  const progress = slides.length > 1 ? (state.slideIndex / (slides.length - 1)) * 100 : 0;
  const hasNext = state.clickStep < state.clickSequence.length || state.slideIndex < slides.length - 1;

  return (
    <div
      className="fixed inset-0 z-50 bg-black flex flex-col"
      style={{ fontFamily: 'system-ui, sans-serif' }}
      tabIndex={0}
    >
      {/* Slide area */}
      <div
        className="flex-1 flex items-center justify-center overflow-hidden bg-black"
        onClick={handleClick}
        style={{ cursor: hasNext ? 'pointer' : 'default' }}
      >
        {/* Scale wrapper */}
        <div
          className="deck-scale-root"
          style={{ transform: `scale(${scale})`, transformOrigin: 'center center' }}
        >
          <div className="deck-container" style={{ position: 'relative', width: 960, height: 540, overflow: 'hidden' }}>
            {slides.map((slide, i) => {
              const isActive = i === state.slideIndex;
              const isExiting = i === prevIndex;
              const transClass = isTransitioning
                ? getTransitionClass(slide, isActive, isExiting)
                : '';

              return (
                <div
                  key={slide.id}
                  ref={el => {
                    if (el) slideRefs.current.set(i, el);
                    else slideRefs.current.delete(i);
                  }}
                  className={`ppt-slide ${isActive ? 'active' : ''} ${isExiting ? 'exiting' : ''} ${transClass}`}
                  dangerouslySetInnerHTML={{ __html: slide.html }}
                />
              );
            })}
          </div>
        </div>
      </div>

      {/* Bottom chrome */}
      <div className="h-8 flex items-center justify-between px-4 bg-black/80 backdrop-blur-sm shrink-0">
        {/* Slide dots */}
        <div className="flex items-center gap-1">
          {slides.map((_, i) => (
            <button
              key={i}
              onClick={e => { e.stopPropagation(); goToSlide(i, i > state.slideIndex ? 'forward' : 'back'); }}
              className="transition-all rounded-full"
              style={{
                width: i === state.slideIndex ? 20 : 6,
                height: 6,
                background: i === state.slideIndex ? '#ffffff' : 'rgba(255,255,255,0.3)',
              }}
            />
          ))}
        </div>

        {/* Slide counter */}
        <div className="text-xs text-white/50 select-none tabular-nums">
          {state.slideIndex + 1} / {slides.length}
          {state.clickSequence.length > 0 && (
            <span className="ml-2 text-white/30">
              {state.clickStep}/{state.clickSequence.length}
            </span>
          )}
        </div>

        {/* Close */}
        {onClose && (
          <button
            onClick={e => { e.stopPropagation(); onClose(); }}
            className="text-xs text-white/40 hover:text-white/80 transition-colors px-2"
          >
            ESC
          </button>
        )}
      </div>

      {/* Progress bar */}
      <div className="h-0.5 bg-white/10 shrink-0">
        <div
          className="h-full bg-white/40 transition-all duration-300"
          style={{ width: `${progress}%` }}
        />
      </div>
    </div>
  );
}
