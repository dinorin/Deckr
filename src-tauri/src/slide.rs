pub fn system_prompt(notes: &str) -> String {
    format!(r#"You are Deckr AI, an expert presentation designer and content strategist.

Your job is to create stunning, professional presentation slides using self-contained HTML/CSS at 960×540px (16:9).

## INTERNAL NOTES (from previous sessions)
{}

## WORKFLOW

### Creating a new deck (first request):
Call `render_deck` with ALL slides at once. Then call `send_message` with a brief summary.

### Editing existing slides:
- Use `edit_slide` for targeted changes to specific slides.
- Use `render_deck` only when rebuilding from scratch or changing the whole theme.
- Always end with `send_message`.

## SLIDE HTML RULES

Each slide is a standalone HTML snippet rendered in a 960×540px div.

Design principles:
- Use ONLY inline styles — no external CSS classes
- Include Google Fonts via <link> in each slide that uses them
- Dark themes feel premium: dark backgrounds with vibrant accent colors
- Strong typographic hierarchy: one bold headline, supporting text, minimal body copy
- Generous whitespace — crowded slides look amateur
- Max 40-50 words per slide (except detailed content slides)
- Use modern gradients, subtle textures for backgrounds
- Consistent visual language across the deck

Slide types and their purpose:
- **title**: Opening slide. Large centered title, subtitle, presenter name
- **content**: Heading + body text or key points
- **bullets**: Numbered or icon-prefixed list (max 5-6 items)
- **two-column**: Two equal panels for comparison or split content
- **quote**: Large pull quote, author attribution
- **closing**: Thank you, Q&A, contact info
- **image**: Placeholder with caption (use CSS gradient as background)

## EXAMPLE HTML (title slide, dark modern theme):

```html
<div style="width:960px;height:540px;background:linear-gradient(135deg,#0f0f1f 0%,#1a1040 50%,#0f0f1f 100%);display:flex;flex-direction:column;align-items:center;justify-content:center;font-family:'Montserrat',sans-serif;padding:80px;box-sizing:border-box;position:relative;overflow:hidden;">
  <link href="https://fonts.googleapis.com/css2?family=Montserrat:wght@400;700;900&display=swap" rel="stylesheet">
  <div style="position:absolute;top:0;left:0;right:0;bottom:0;background:radial-gradient(ellipse at 30% 50%,rgba(99,102,241,0.15) 0%,transparent 60%);pointer-events:none;"></div>
  <div style="font-size:12px;letter-spacing:5px;color:#6366f1;text-transform:uppercase;margin-bottom:20px;font-weight:600;">DECKR PRESENTATION</div>
  <h1 style="font-size:60px;font-weight:900;color:#ffffff;text-align:center;line-height:1.05;margin:0 0 24px 0;letter-spacing:-2px;">Your Title Here</h1>
  <p style="font-size:18px;color:#94a3b8;text-align:center;margin:0 0 40px 0;">Subtitle or tagline goes here</p>
  <div style="width:60px;height:3px;background:linear-gradient(90deg,#6366f1,#8b5cf6);border-radius:2px;"></div>
</div>
```

Always generate visually impressive, professional slides. Vary layouts between slides to maintain visual interest."#, notes)
}

pub fn tools_openai() -> serde_json::Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "render_deck",
                "description": "Create or completely replace the entire slide deck with new HTML slides",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "title": {
                            "type": "string",
                            "description": "The presentation title"
                        },
                        "theme": {
                            "type": "object",
                            "description": "Visual theme of the deck",
                            "properties": {
                                "primaryColor": { "type": "string" },
                                "secondaryColor": { "type": "string" },
                                "backgroundColor": { "type": "string" },
                                "textColor": { "type": "string" },
                                "fontFamily": { "type": "string" },
                                "style": {
                                    "type": "string",
                                    "enum": ["modern", "minimal", "bold", "corporate", "creative"]
                                }
                            },
                            "required": ["primaryColor", "secondaryColor", "backgroundColor", "textColor", "fontFamily", "style"]
                        },
                        "slides": {
                            "type": "array",
                            "description": "Array of slides",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": { "type": "string", "description": "Unique slide ID (e.g. s1, s2)" },
                                    "type": {
                                        "type": "string",
                                        "enum": ["title", "content", "bullets", "two-column", "quote", "closing", "image"]
                                    },
                                    "html": { "type": "string", "description": "Complete self-contained HTML for this slide at 960x540px" }
                                },
                                "required": ["id", "type", "html"]
                            }
                        }
                    },
                    "required": ["title", "theme", "slides"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "edit_slide",
                "description": "Replace the HTML of a specific slide",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "slideId": { "type": "string", "description": "The ID of the slide to edit" },
                        "html": { "type": "string", "description": "The new complete HTML for the slide" }
                    },
                    "required": ["slideId", "html"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "send_message",
                "description": "Send a message to the user (summary, confirmation, suggestions, questions)",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"]
                }
            }
        }
    ])
}

pub fn tools_gemini() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "render_deck",
            "description": "Create or completely replace the entire slide deck",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "title": { "type": "STRING" },
                    "theme": {
                        "type": "OBJECT",
                        "properties": {
                            "primaryColor": { "type": "STRING" },
                            "secondaryColor": { "type": "STRING" },
                            "backgroundColor": { "type": "STRING" },
                            "textColor": { "type": "STRING" },
                            "fontFamily": { "type": "STRING" },
                            "style": { "type": "STRING" }
                        },
                        "required": ["primaryColor", "secondaryColor", "backgroundColor", "textColor", "fontFamily", "style"]
                    },
                    "slides": {
                        "type": "ARRAY",
                        "items": {
                            "type": "OBJECT",
                            "properties": {
                                "id": { "type": "STRING" },
                                "type": { "type": "STRING" },
                                "html": { "type": "STRING" }
                            },
                            "required": ["id", "type", "html"]
                        }
                    }
                },
                "required": ["title", "theme", "slides"]
            }
        },
        {
            "name": "edit_slide",
            "description": "Replace the HTML of a specific slide by its ID",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "slideId": { "type": "STRING" },
                    "html": { "type": "STRING" }
                },
                "required": ["slideId", "html"]
            }
        },
        {
            "name": "send_message",
            "description": "Send a message to the user",
            "parameters": {
                "type": "OBJECT",
                "properties": {
                    "message": { "type": "STRING" }
                },
                "required": ["message"]
            }
        }
    ])
}
