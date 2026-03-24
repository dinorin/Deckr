use futures::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use serde_json::json;
use tauri::Emitter;

use super::{AgentContext, AgentMessage, DeckTheme, GeneratedDeck, GeneratedSlide, SlideAnimationPlan, SlideOutline, call_llm};
use crate::settings::AppSettings;
use crate::tools::ImageResult;

#[derive(Serialize, Clone)]
pub struct SlideReadyPayload {
    pub index: usize,
    pub id: String,
    pub slide_type: String,
    pub html: String,
}

// Prompt for generating a SINGLE slide's HTML
const SLIDE_SYSTEM_PROMPT: &str = r##"You are a world-class slide designer. Output ONLY raw HTML for one 960×540 slide. No markdown, no explanation, no code fences.

## Slide wrapper
<div class="ppt-slide" data-slide-index="N" data-bg-color="#hex" data-transition="fade|push|wipe|none"
     style="position:relative;width:960px;height:540px;overflow:hidden;font-family:'Font',sans-serif;box-sizing:border-box;color:#fff;">

## Typography & Content Length
- CRITICAL: Keep text extremely concise! Max 15-20 words per text block. Viewers do not read paragraphs.
- ALWAYS use proper semantic tags for text: <h1>, <h2>, <h3>, <p>, <li>.
- STRICT Font Size Rules for 960x540 canvas (set via inline style AND data-ppt-font-size):
  - <h1>: 48px to 60px (Main titles)
  - <h2>: 36px to 44px (Subtitles)
  - <h3>: 28px to 32px (Section headers)
  - <p>, <li>: 20px to 24px (Body text)
- Do NOT make text too massive or too tiny. It must fit cleanly inside the 960x540 slide.

## Design freedom — make it beautiful
- Use ANY CSS layout: flexbox, grid, absolute positioning, overlapping layers, full-bleed images
- Mix bold typography with imagery. Text can sit on top of images with a scrim/blur
- Try dramatic compositions: giant number + small label, full-screen photo + corner text, diagonal split
- Decorative elements: gradient blobs, geometric shapes, thin lines, icon grids, glowing orbs
- Google Fonts via @import or system fonts (Montserrat, Inter, Playfair Display, Space Grotesk, etc.)
- Font Awesome icons: <i class="fa-solid fa-NAME"></i>

## Layers (render in order)
1. Background: full-bleed image or gradient (always cover the whole slide)
2. Overlay / scrim for text readability (rgba or backdrop-filter:blur)
3. Decorative shapes, accent lines, blobs
4. Images / illustrations (panels, circles, cards, floating)
5. Text content — clear, readable, not cramped

## Image strategy
- CRITICAL: Do NOT just use one background image for every slide. Mix it up!
- Use <img> tags for side-by-side layouts, circular profile pics, grid galleries, or floating elements.
- DDG images provided → use their URLs directly (src="" or background-image).
- Need a specific image not provided → AI placeholder:
  <img class="ai-gen-image" src="" data-prompt="detailed description" style="...">
- AI background: <div style="background-image:url('')" class="ai-gen-image" data-prompt="..." ...>
- ALWAYS use at least one image per slide, but vary its placement and size.

## Layout & Positioning Rules
- CRITICAL: Do NOT use `transform` for centering or layout (e.g., no `left:50%; transform:translateX(-50%)`). 
- Reason: The Animation system uses `transform` and will OVERWRITE your layout transforms, causing elements to shift or disappear.
- ALWAYS use Flexbox, Grid, or absolute `top/left/right/bottom` values for positioning.
- To center: use `display:flex; justify-content:center; align-items:center;` on the slide or a full-width container.
- Everything must fit within 960x540. Do NOT use `vw` or `vh` units.

## Animated element pattern
CRITICAL: You MUST wrap EVERY element inside an animation <div>. Do not drop the wrapper!
Apply all positioning and layout styles (absolute coords, flex, grid areas, margins) to the WRAPPER <div>, NOT the inner text tag.
<div class="ppt-element ppt-hidden ppt-ANIMATION" data-click="N" data-duration="Xms" data-ppt-animation="ANIMATION" style="...">
  <h2 data-ppt-font-size="N" data-ppt-bold="true|false" data-ppt-color="#hex" data-ppt-align="left|center|right" data-ppt-font="Font">content</h2>
</div>

## Animation names
ppt-appear | ppt-fade-in | ppt-fly-in-bottom | ppt-fly-in-top | ppt-fly-in-left | ppt-fly-in-right | ppt-zoom-in | ppt-bounce-in | ppt-float-in | ppt-wipe-left | ppt-split | ppt-swivel

## Rules
- data-click="0" = always visible; data-click="N" = revealed on Nth click
- data-ppt-* attributes on text tags are required for PPTX export — always include them
- All text elements must have explicit color in inline style (e.g. color:#fff) — never rely on inherited color
- Max ~50 words of text per slide — less is more
- Each slide should feel unique: vary composition, image placement, typography scale"##;

pub async fn run(
    app: &tauri::AppHandle,
    settings: &AppSettings,
    ctx: &AgentContext,
    outline: &[SlideOutline],
    theme: &DeckTheme,
    animation_plan: &[SlideAnimationPlan],
    images: &[ImageResult],
) -> Result<GeneratedDeck, String> {
    let total = animation_plan.len();

    // Build unordered futures — each slide runs independently
    let mut futs: FuturesUnordered<_> = animation_plan.iter().enumerate().map(|(i, slide_plan)| {
        let settings = settings.clone();
        let outline_slide = outline.get(slide_plan.index).or_else(|| outline.get(i)).cloned();
        let slide_plan = slide_plan.clone();
        let theme = theme.clone();
        let ctx_language = ctx.language.clone();
        let images = images.to_vec();

        async move {
            generate_single_slide(
                &settings,
                i,
                total,
                &slide_plan,
                outline_slide.as_ref(),
                &theme,
                &ctx_language,
                &images,
            ).await.map(|slide| (i, slide))
        }
    }).collect();

    // Collect results as they complete — emit each slide immediately
    let mut slides: Vec<Option<GeneratedSlide>> = vec![None; total];
    while let Some(result) = futs.next().await {
        match result {
            Ok((i, slide)) => {
                let _ = app.emit("slide-ready", SlideReadyPayload {
                    index: i,
                    id: slide.id.clone(),
                    slide_type: slide.slide_type.clone(),
                    html: slide.html.clone(),
                });
                slides[i] = Some(slide);
            }
            Err(e) => return Err(e),
        }
    }

    let slides: Vec<GeneratedSlide> = slides.into_iter().flatten().collect();

    let title = outline.first()
        .map(|s| s.title.clone())
        .unwrap_or_else(|| ctx.topic.clone());

    let master_html = build_master_html(&title, &slides);

    let theme_json = json!({
        "primaryColor": theme.primary_color,
        "secondaryColor": theme.secondary_color,
        "backgroundColor": theme.bg_color,
        "textColor": theme.text_color,
        "fontFamily": theme.font_family,
        "style": theme.style,
    });

    Ok(GeneratedDeck {
        title,
        theme: theme_json,
        slides,
        master_html,
        coach_message: String::new(),
    })
}

async fn generate_single_slide(
    settings: &AppSettings,
    index: usize,
    total: usize,
    slide_plan: &SlideAnimationPlan,
    outline_slide: Option<&SlideOutline>,
    theme: &DeckTheme,
    language: &str,
    images: &[ImageResult],
) -> Result<GeneratedSlide, String> {
    let empty_tools = json!([]);

    let elements_desc = slide_plan.elements.iter().map(|el| {
        format!(
            "  [{type}] \"{content}\" | animation:ppt-{anim} click={click} dur={dur}ms",
            type = el.element_type,
            content = el.content,
            anim = el.animation,
            click = el.click_order,
            dur = el.duration_ms,
        )
    }).collect::<Vec<_>>().join("\n");

    let slide_type = outline_slide.map(|o| o.slide_type.as_str()).unwrap_or("content");
    let slide_title = outline_slide.map(|o| o.title.as_str()).unwrap_or("");
    let transition = &slide_plan.transition;
    let bg = &slide_plan.bg_color;

    // Build image list — rotate starting index per slide for variety
    let image_block = if images.is_empty() {
        String::new()
    } else {
        let len = images.len();
        // Shift start by index, so each slide gets a different primary set
        let start = (index * 3) % len;
        let mut lines: Vec<String> = Vec::new();
        // Give up to 6 images per slide for more choice
        for i in 0..len.min(6) {
            let img = &images[(start + i) % len];
            lines.push(format!(
                "  - \"{desc}\" {w}×{h} | URL: {url}",
                desc = img.description.replace('"', "'"),
                w = img.width, h = img.height,
                url = img.url,
            ));
        }
        format!("\nAvailable images for this slide:\n{}\n\nCRITICAL: DO NOT use the same image on every slide. Pick different images. DO NOT just use images as backgrounds; place them in <img> tags in columns, grids, or floating elements.", lines.join("\n"))
    };

    let user_msg = format!(
        r#"Design slide {idx} of {total} — type: {stype}, title: "{title}"
Language: {lang}

Theme: {style} | primary:{pri} | accent:{acc} | bg:{bg} | font:{font}
Transition: {tr}
{images}
Content to show (animate each per spec, design layout freely):
{elements}

Make it visually stunning. Output ONLY raw HTML starting with <div class="ppt-slide""#,
        idx   = index + 1,
        total = total,
        stype = slide_type,
        title = slide_title,
        lang  = language,
        style = theme.style,
        pri   = theme.primary_color,
        acc   = theme.accent_color,
        bg    = bg,
        font  = theme.font_family,
        tr    = transition,
        images = image_block,
        elements = elements_desc,
    );

    let history = vec![AgentMessage { role: "user".to_string(), content: user_msg }];
    let resp = call_llm(settings, SLIDE_SYSTEM_PROMPT, &history, &empty_tools).await
        .map_err(|e| e.to_string())?;

    let raw = resp.text.unwrap_or_default();
    let raw = raw.trim();

    let html = if raw.contains("ppt-slide") {
        raw.to_string()
    } else {
        let extracted = extract_html_div(raw);
        if extracted.contains("ppt-slide") {
            extracted
        } else {
            build_fallback_slide(slide_plan, outline_slide, theme, index)
        }
    };

    let slide_id = format!("s{}", index + 1);
    Ok(GeneratedSlide {
        id: slide_id,
        slide_type: slide_type.to_string(),
        html,
    })
}

fn extract_html_div(text: &str) -> String {
    if let Some(start) = text.find("<div") {
        if let Some(end) = text.rfind("</div>") {
            return text[start..end + 6].to_string();
        }
        return text[start..].to_string();
    }
    if let Some(start) = text.find("```html") {
        let after = &text[start + 7..];
        let end = after.find("```").unwrap_or(after.len());
        return after[..end].trim().to_string();
    }
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        let end = after.find("```").unwrap_or(after.len());
        return after[..end].trim().to_string();
    }
    text.to_string()
}

/// Build a minimal but functional slide when LLM fails to produce proper HTML.
fn build_fallback_slide(
    plan: &SlideAnimationPlan,
    outline: Option<&SlideOutline>,
    theme: &DeckTheme,
    index: usize,
) -> String {
    let bg = &plan.bg_color;
    let font = &theme.font_family;
    let accent = &theme.accent_color;

    let mut elements_html = String::new();

    for el in &plan.elements {
        let click_attr = if el.click_order == 0 {
            String::new()
        } else {
            format!(
                r#" class="ppt-element ppt-hidden ppt-{anim}" data-click="{click}" data-duration="{dur}" data-ppt-animation="{anim}""#,
                anim = el.animation,
                click = el.click_order,
                dur = el.duration_ms,
            )
        };

        let color = &el.color;
        let fs = el.font_size;
        let bold = if el.bold { "font-weight:900;" } else { "font-weight:400;" };
        let align = &el.align;
        let content = html_escape(&el.content);

        elements_html.push_str(&format!(
            r#"<div{click} style="position:absolute;left:{x}px;top:{y}px;width:{w}px;height:{h}px;overflow:hidden;">
  <p data-ppt-font-size="{fs}" data-ppt-bold="{bold_attr}" data-ppt-color="{color}" data-ppt-align="{align}" data-ppt-font="{font}"
     style="margin:0;font-size:{fs}px;{bold}color:{color};font-family:'{font}',sans-serif;text-align:{align};line-height:1.3;">{content}</p>
</div>
"#,
            click = click_attr,
            x = el.x, y = el.y, w = el.width, h = el.height,
            fs = fs,
            bold_attr = el.bold,
            color = color,
            align = align,
            font = font,
            bold = bold,
            content = content,
        ));
    }

    let _ = outline; // available if needed for future enhancements

    format!(
        r#"<div class="ppt-slide" data-slide-index="{idx}" data-bg-color="{bg}" data-transition="{tr}"
     style="position:relative;width:960px;height:540px;overflow:hidden;font-family:'{font}',sans-serif;">
  <div class="ppt-bg-element" style="position:absolute;inset:0;background:{bg};"></div>
  <div class="ppt-bg-element" style="position:absolute;bottom:0;left:0;right:0;height:4px;background:{accent};"></div>
{elements}
</div>"#,
        idx = index,
        bg = bg,
        tr = plan.transition,
        font = font,
        accent = accent,
        elements = elements_html,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

/// Assemble all slide HTML fragments into a single self-contained presentation HTML file.
/// Uses Web Animations API (no CSS keyframes) for element entrance animations.
pub fn build_master_html(title: &str, slides: &[GeneratedSlide]) -> String {
    let slides_html: String = slides.iter().enumerate().map(|(i, s)| {
        let html = if s.html.contains("data-slide-index") {
            s.html.clone()
        } else {
            s.html.replacen("<div", &format!("<div data-slide-index=\"{}\"", i), 1)
        };
        html
    }).collect::<Vec<_>>().join("\n");

    format!(r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>{title}</title>
<link rel="stylesheet" href="https://cdnjs.cloudflare.com/ajax/libs/font-awesome/6.5.1/css/all.min.css" crossorigin="anonymous">
<style>
*,*::before,*::after{{box-sizing:border-box;margin:0;padding:0}}
html,body{{width:100%;height:100%;overflow:hidden;background:#111;color:#fff;display:flex;align-items:center;justify-content:center}}

#deck{{position:relative;width:960px;height:540px;transform-origin:center center;}}

.ppt-slide{{position:absolute;inset:0;width:960px;height:540px;overflow:hidden;display:none;color:#fff;}}
.ppt-slide.active{{display:block;z-index:10;}}
.ppt-slide.exiting{{display:block;z-index:5;}}

.ppt-element{{display:block;position:relative;box-sizing:border-box;}}

/* JS animations: elements start hidden via this class; JS removes it and calls element.animate() */
.ppt-element.ppt-hidden{{opacity:0!important;visibility:hidden!important;pointer-events:none;}}

/* Slide transitions */
.ppt-slide.tr-fade-enter{{animation:deckFadeIn .4s ease forwards;}}
.ppt-slide.tr-fade-exit{{animation:deckFadeOut .4s ease forwards;}}
@keyframes deckFadeIn{{from{{opacity:0;}}to{{opacity:1;}}}}
@keyframes deckFadeOut{{from{{opacity:1;}}to{{opacity:0;}}}}

.ppt-slide.tr-push-enter{{animation:deckPushEnter .45s cubic-bezier(.4,0,.2,1) forwards;}}
.ppt-slide.tr-push-exit{{animation:deckPushExit .45s cubic-bezier(.4,0,.2,1) forwards;}}
@keyframes deckPushEnter{{from{{transform:translateX(100%);}}to{{transform:translateX(0);}}}}
@keyframes deckPushExit{{from{{transform:translateX(0);}}to{{transform:translateX(-100%);}}}}

.ppt-slide.tr-push-back-enter{{animation:deckPushBackEnter .45s cubic-bezier(.4,0,.2,1) forwards;}}
.ppt-slide.tr-push-back-exit{{animation:deckPushBackExit .45s cubic-bezier(.4,0,.2,1) forwards;}}
@keyframes deckPushBackEnter{{from{{transform:translateX(-100%);}}to{{transform:translateX(0);}}}}
@keyframes deckPushBackExit{{from{{transform:translateX(0);}}to{{transform:translateX(100%);}}}}

.ppt-slide.tr-wipe-enter{{animation:deckWipeEnter .5s ease-out forwards;clip-path:inset(0 100% 0 0);}}
@keyframes deckWipeEnter{{from{{clip-path:inset(0 100% 0 0);}}to{{clip-path:inset(0 0 0 0);}}}}

#progress{{position:fixed;bottom:0;left:0;height:2px;background:rgba(255,255,255,.35);transition:width .3s ease;z-index:100;}}
</style>
</head>
<body>
<div id="deck">
{slides_html}
</div>
<div id="progress"></div>
<script>
(function(){{
  /* ── Web Animations API — entrance animations ─────────────────────────── */
  var ANIMS = {{
    'appear':        function(e,d){{e.animate([{{opacity:0}},{{opacity:1}}],{{duration:Math.min(d,100),fill:'forwards'}});}},
    'fade-in':       function(e,d){{e.animate([{{opacity:0}},{{opacity:1}}],{{duration:d,easing:'ease',fill:'forwards'}});}},
    'fly-in-bottom': function(e,d){{e.animate([{{opacity:0,transform:'translateY(110%)'}},{{opacity:1,transform:'translateY(0)'}}],{{duration:d,easing:'cubic-bezier(.25,.46,.45,.94)',fill:'forwards'}});}},
    'fly-in-top':    function(e,d){{e.animate([{{opacity:0,transform:'translateY(-110%)'}},{{opacity:1,transform:'translateY(0)'}}],{{duration:d,easing:'cubic-bezier(.25,.46,.45,.94)',fill:'forwards'}});}},
    'fly-in-left':   function(e,d){{e.animate([{{opacity:0,transform:'translateX(-110%)'}},{{opacity:1,transform:'translateX(0)'}}],{{duration:d,easing:'cubic-bezier(.25,.46,.45,.94)',fill:'forwards'}});}},
    'fly-in-right':  function(e,d){{e.animate([{{opacity:0,transform:'translateX(110%)'}},{{opacity:1,transform:'translateX(0)'}}],{{duration:d,easing:'cubic-bezier(.25,.46,.45,.94)',fill:'forwards'}});}},
    'zoom-in':       function(e,d){{e.animate([{{opacity:0,transform:'scale(0.3)'}},{{opacity:1,transform:'scale(1)'}}],{{duration:d,easing:'cubic-bezier(.175,.885,.32,1.275)',fill:'forwards'}});}},
    'bounce-in':     function(e,d){{e.animate([{{opacity:0,transform:'scale(0.3)'}},{{opacity:1,transform:'scale(1.08)'}},{{opacity:1,transform:'scale(0.94)'}},{{opacity:1,transform:'scale(1)'}}],{{duration:d,fill:'forwards'}});}},
    'float-in':      function(e,d){{e.animate([{{opacity:0,transform:'translateY(36px)'}},{{opacity:1,transform:'translateY(0)'}}],{{duration:d,easing:'cubic-bezier(.22,1,.36,1)',fill:'forwards'}});}},
    'wipe-left':     function(e,d){{e.style.opacity='1';e.animate([{{clipPath:'inset(0 100% 0 0)'}},{{clipPath:'inset(0 0% 0 0)'}}],{{duration:d,easing:'ease-out',fill:'forwards'}});}},
    'split':         function(e,d){{e.style.opacity='1';e.animate([{{clipPath:'inset(50% 0)'}},{{clipPath:'inset(0% 0)'}}],{{duration:d,easing:'ease-out',fill:'forwards'}});}},
    'swivel':        function(e,d){{e.animate([{{opacity:0,transform:'perspective(800px) rotateY(-90deg)'}},{{opacity:1,transform:'perspective(800px) rotateY(0)'}}],{{duration:d,easing:'cubic-bezier(.4,0,.2,1)',fill:'forwards'}});}}
  }};

  function revealEl(el){{
    var anim = el.getAttribute('data-ppt-animation') || 'fade-in';
    var dur  = parseInt(el.getAttribute('data-duration') || '500', 10);
    el.getAnimations().forEach(function(a){{a.cancel();}});
    el.classList.remove('ppt-hidden');
    el.style.visibility = 'visible';
    (ANIMS[anim] || ANIMS['fade-in'])(el, dur);
  }}

  function hideEl(el){{
    el.getAnimations().forEach(function(a){{a.cancel();}});
    el.style.opacity = '0';
    el.style.visibility = 'hidden';
    el.style.transform = '';
    el.style.clipPath = '';
    el.classList.add('ppt-hidden');
  }}

  /* ── Deck state ─────────────────────────────────────────────────────────── */
  var deck = document.getElementById('deck');
  var progress = document.getElementById('progress');
  var slides = Array.from(deck.querySelectorAll('.ppt-slide'));
  var total = slides.length;
  var cur = 0;
  var clickStep = 0;
  var clickSequence = [];
  var transitioning = false;

  function getSequence(slide) {{
    var s = new Set();
    slide.querySelectorAll('[data-click]').forEach(function(el){{
      var c = parseInt(el.getAttribute('data-click'), 10);
      if(c > 0) s.add(c);
    }});
    return Array.from(s).sort(function(a,b){{return a-b;}});
  }}

  function updateProgress(){{
    progress.style.width = ((cur + 1) / total * 100) + '%';
  }}

  function showSlide(n, dir){{
    if(transitioning || n === cur) return;
    var prev = slides[cur];
    var next = slides[n];
    var tr = next.getAttribute('data-transition') || 'fade';
    transitioning = true;

    // Reset animated elements on incoming slide
    next.querySelectorAll('[data-click]').forEach(function(el){{
      var c = parseInt(el.getAttribute('data-click'),10);
      if(c > 0) {{
         hideEl(el);
      }} else if (c === 0) {{
         el.classList.remove('ppt-hidden');
         el.style.opacity = '';
         el.style.visibility = '';
         revealEl(el);
      }}
    }});

    if(tr === 'none'){{
      prev.classList.remove('active');
      next.classList.add('active');
      transitioning = false;
    }} else {{
      var enterClass = dir >= 0 ? 'tr-'+tr+'-enter' : 'tr-'+tr+'-back-enter';
      var exitClass  = dir >= 0 ? 'tr-'+tr+'-exit'  : 'tr-'+tr+'-back-exit';
      prev.classList.add('exiting', exitClass);
      next.classList.add('active', enterClass);
      setTimeout(function(){{
        prev.classList.remove('active','exiting',exitClass);
        next.classList.remove(enterClass);
        transitioning = false;
      }}, 480);
    }}

    cur = n;
    clickSequence = getSequence(next);
    clickStep = 0;
    updateProgress(); notifyParent();
  }}

  function revealNext(){{
    if (clickStep < clickSequence.length) {{
      var nextNum = clickSequence[clickStep];
      var targets = slides[cur].querySelectorAll('[data-click="'+nextNum+'"]');
      targets.forEach(function(el){{ revealEl(el); }});
      clickStep++;
      return true;
    }}
    return false;
  }}

  function advance(){{
    if(transitioning) return;
    if(!revealNext()) {{ if(cur < total-1) showSlide(cur+1, 1); }}
  }}
  function retreat(){{
    if(transitioning || cur <= 0) return;
    showSlide(cur-1, -1);
  }}
  function notifyParent(){{
    try{{ window.parent.postMessage({{type:'slideChange',index:cur,total:total}},'*'); }}catch(e){{}}
  }}

  /* ── Message bus ──────────────────────────────────────────────────────── */
  window.addEventListener('message', function(e){{
    if(!e.data) return;
    var d = e.data;
    if(d.type==='goto')       showSlide(d.index, d.index>cur ? 1 : -1);
    if(d.type==='advance')    advance();
    if(d.type==='retreat')    retreat();
    if(d.type==='updateImage'){{
      var p = d.prompt, u = d.url;
      deck.querySelectorAll('img.ai-gen-image').forEach(function(el){{
        if(el.getAttribute('data-prompt')===p) el.src = u;
      }});
      deck.querySelectorAll('div.ai-gen-image').forEach(function(el){{
        if(el.getAttribute('data-prompt')===p) el.style.backgroundImage='url('+u+')';
      }});
    }}
  }});

  deck.addEventListener('click', advance);
  document.addEventListener('keydown', function(e){{
    if(e.key==='ArrowRight'||e.key===' '||e.key==='PageDown') advance();
    if(e.key==='ArrowLeft' ||e.key==='PageUp') retreat();
    if(e.key==='Home') showSlide(0,-1);
    if(e.key==='End')  showSlide(total-1,1);
  }});

  function scaleToFit(){{
    var s = Math.min(window.innerWidth/960, window.innerHeight/540);
    deck.style.transform = 'scale('+s+')';
  }}
  window.addEventListener('resize', scaleToFit);
  scaleToFit();

  /* ── Init ─────────────────────────────────────────────────────────────── */
  slides.forEach(function(slide){{
    slide.querySelectorAll('[data-click]').forEach(function(el){{
      var c = parseInt(el.getAttribute('data-click'),10);
      if(c > 0) hideEl(el);
    }});
  }});
  if(slides[0]) {{
     slides[0].classList.add('active');
     clickSequence = getSequence(slides[0]);
     slides[0].querySelectorAll('[data-click="0"]').forEach(function(el) {{ revealEl(el); }});
  }}
  updateProgress(); notifyParent();
}})();
</script>
</body>
</html>"##,
        title = title,
        slides_html = slides_html,
    )
}
