use std::collections::HashMap;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use super::animation_map::map_animation;

// EMU conversion: 1 HTML px at 960×540 → 9525 EMU
const EMU_PER_PX: u32 = 9525;
// Slide dimensions in EMU
const SLIDE_W: u32 = 9_144_000;
const SLIDE_H: u32 = 5_143_500;

#[derive(Debug, Clone)]
pub struct PptxSlide {
    pub bg_color: String,
    pub transition: String,
    pub elements: Vec<PptxElement>,
}

#[derive(Debug, Clone)]
pub struct PptxElement {
    pub id: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub content: String,
    pub font_size: u32,    // points (hundredths for PPT: multiply by 100)
    pub bold: bool,
    pub italic: bool,
    pub color: String,     // hex without #
    pub align: String,
    pub font_family: String,
    pub animation: String,
    pub click_order: u32,
    pub duration_ms: u32,
    pub element_type: String,
}

pub fn build_pptx(title: &str, slides: &[PptxSlide]) -> Result<Vec<u8>, String> {
    let buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(content_types_xml(slides.len()).as_bytes()).map_err(|e| e.to_string())?;

    // _rels/.rels
    zip.start_file("_rels/.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(root_rels_xml().as_bytes()).map_err(|e| e.to_string())?;

    // docProps/app.xml
    zip.start_file("docProps/app.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(app_xml(title).as_bytes()).map_err(|e| e.to_string())?;

    // docProps/core.xml
    zip.start_file("docProps/core.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(core_xml(title).as_bytes()).map_err(|e| e.to_string())?;

    // ppt/presentation.xml
    zip.start_file("ppt/presentation.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(presentation_xml(slides.len()).as_bytes()).map_err(|e| e.to_string())?;

    // ppt/_rels/presentation.xml.rels
    zip.start_file("ppt/_rels/presentation.xml.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(presentation_rels_xml(slides.len()).as_bytes()).map_err(|e| e.to_string())?;

    // ppt/slideMasters/slideMaster1.xml
    zip.start_file("ppt/slideMasters/slideMaster1.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(slide_master_xml().as_bytes()).map_err(|e| e.to_string())?;

    // ppt/slideMasters/_rels/slideMaster1.xml.rels
    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(slide_master_rels_xml().as_bytes()).map_err(|e| e.to_string())?;

    // ppt/slideLayouts/slideLayout1.xml
    zip.start_file("ppt/slideLayouts/slideLayout1.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(slide_layout_xml().as_bytes()).map_err(|e| e.to_string())?;

    // ppt/slideLayouts/_rels/slideLayout1.xml.rels
    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", opts).map_err(|e| e.to_string())?;
    zip.write_all(slide_layout_rels_xml().as_bytes()).map_err(|e| e.to_string())?;

    // ppt/theme/theme1.xml
    zip.start_file("ppt/theme/theme1.xml", opts).map_err(|e| e.to_string())?;
    zip.write_all(theme_xml().as_bytes()).map_err(|e| e.to_string())?;

    // Each slide
    for (i, slide) in slides.iter().enumerate() {
        let n = i + 1;

        zip.start_file(format!("ppt/slides/slide{}.xml", n), opts).map_err(|e| e.to_string())?;
        zip.write_all(slide_xml(n as u32, slide).as_bytes()).map_err(|e| e.to_string())?;

        zip.start_file(format!("ppt/slides/_rels/slide{}.xml.rels", n), opts).map_err(|e| e.to_string())?;
        zip.write_all(slide_rels_xml().as_bytes()).map_err(|e| e.to_string())?;
    }

    let result = zip.finish().map_err(|e| e.to_string())?;
    Ok(result.into_inner())
}

// ─── Parse slide HTML to extract PptxSlide ────────────────────────────────────

pub fn parse_slide_html(html: &str, slide_index: usize) -> PptxSlide {
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);

    // Get slide-level attributes
    let slide_sel = Selector::parse(".ppt-slide").unwrap();
    let mut bg_color = "0f0f1f".to_string();
    let mut transition = "fade".to_string();

    if let Some(slide_el) = document.select(&slide_sel).next() {
        if let Some(bg) = slide_el.value().attr("data-bg-color") {
            bg_color = bg.trim_start_matches('#').to_string();
        }
        if let Some(tr) = slide_el.value().attr("data-transition") {
            transition = tr.to_string();
        }
    }

    // Get animated elements
    let elem_sel = Selector::parse(".ppt-element").unwrap();
    let text_sel = Selector::parse("[data-ppt-font-size]").unwrap();

    let mut elements = Vec::new();
    let mut elem_id = 2u32; // Start at 2 (1 is background)

    for el in document.select(&elem_sel) {
        let el_val = el.value();

        // Parse position from style attribute
        let style = el_val.attr("style").unwrap_or("");
        let (x, y, width, height) = parse_position_from_style(style);

        let animation = el_val.attr("data-ppt-animation").unwrap_or("fade-in").to_string();
        let click_order = el_val.attr("data-click")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(1);
        let duration_ms = el_val.attr("data-duration")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(500);
        let element_type = el_val.attr("data-ppt-type").unwrap_or("body").to_string();

        // Get text content and styling from child text element
        let mut content = String::new();
        let mut font_size = 24u32;
        let mut bold = false;
        let mut italic = false;
        let mut color = "ffffff".to_string();
        let mut align = "left".to_string();
        let mut font_family = "Calibri".to_string();

        if let Some(text_el) = el.select(&text_sel).next() {
            let tv = text_el.value();
            font_size = tv.attr("data-ppt-font-size")
                .and_then(|v| v.parse().ok())
                .unwrap_or(24);
            bold = tv.attr("data-ppt-bold").map(|v| v == "true").unwrap_or(false);
            italic = tv.attr("data-ppt-italic").map(|v| v == "true").unwrap_or(false);
            color = tv.attr("data-ppt-color")
                .unwrap_or("#ffffff")
                .trim_start_matches('#')
                .to_string();
            align = tv.attr("data-ppt-align").unwrap_or("left").to_string();
            font_family = tv.attr("data-ppt-font").unwrap_or("Calibri").to_string();
            content = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
        } else {
            // Fallback: get all text
            content = el.text().collect::<Vec<_>>().join(" ").trim().to_string();
        }

        if content.is_empty() { continue; }

        elements.push(PptxElement {
            id: elem_id,
            x, y, width, height,
            content,
            font_size,
            bold,
            italic,
            color,
            align,
            font_family,
            animation,
            click_order,
            duration_ms,
            element_type,
        });
        elem_id += 1;
    }

    PptxSlide { bg_color, transition, elements }
}

fn parse_position_from_style(style: &str) -> (u32, u32, u32, u32) {
    let mut x = 80u32;
    let mut y = 80u32;
    let mut width = 800u32;
    let mut height = 100u32;

    for part in style.split(';') {
        let part = part.trim();
        if let Some((key, val)) = part.split_once(':') {
            let key = key.trim();
            let val = val.trim().trim_end_matches("px").trim();
            if let Ok(n) = val.parse::<f32>() {
                let n = n as u32;
                match key {
                    "left" => x = n,
                    "top" => y = n,
                    "width" => width = n,
                    "height" => height = n,
                    _ => {}
                }
            }
        }
    }

    (x, y, width, height)
}

// ─── XML Generators ───────────────────────────────────────────────────────────

fn px_to_emu(px: u32) -> u32 {
    px * EMU_PER_PX
}

fn slide_xml(n: u32, slide: &PptxSlide) -> String {
    let bg_hex = normalize_hex(&slide.bg_color);
    let shapes = slide.elements.iter().map(|el| shape_xml(el)).collect::<Vec<_>>().join("\n");
    let timing = timing_xml(slide);

    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:bg>
      <p:bgPr>
        <a:solidFill><a:srgbClr val="{bg}"/></a:solidFill>
        <a:effectLst/>
      </p:bgPr>
    </p:bg>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm>
          <a:off x="0" y="0"/>
          <a:ext cx="{sw}" cy="{sh}"/>
          <a:chOff x="0" y="0"/>
          <a:chExt cx="{sw}" cy="{sh}"/>
        </a:xfrm>
      </p:grpSpPr>
{shapes}
    </p:spTree>
  </p:cSld>
{timing}
</p:sld>"#,
        bg = bg_hex,
        sw = SLIDE_W,
        sh = SLIDE_H,
        shapes = shapes,
        timing = timing,
    )
}

fn shape_xml(el: &PptxElement) -> String {
    let x = px_to_emu(el.x);
    let y = px_to_emu(el.y);
    let cx = px_to_emu(el.width);
    let cy = px_to_emu(el.height);
    let sz = el.font_size * 100; // PPT uses hundredths of a point
    let color = normalize_hex(&el.color);
    let bold_attr = if el.bold { r#" b="1""# } else { "" };
    let italic_attr = if el.italic { r#" i="1""# } else { "" };
    let algn = match el.align.as_str() {
        "center" => "ctr",
        "right" => "r",
        _ => "l",
    };

    // Escape XML special chars in content
    let safe_content = el.content
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");

    // Visibility: hidden if animated (click_order > 0)
    let hidden = if el.click_order > 0 { r#" style.visibility="hidden""# } else { "" };

    format!(r#"      <p:sp>
        <p:nvSpPr>
          <p:cNvPr id="{id}" name="elem{id}"/>
          <p:cNvSpPr txBox="1"/>
          <p:nvPr/>
        </p:nvSpPr>
        <p:spPr>
          <a:xfrm>
            <a:off x="{x}" y="{y}"/>
            <a:ext cx="{cx}" cy="{cy}"/>
          </a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:noFill/>
        </p:spPr>
        <p:txBody>
          <a:bodyPr wrap="square" rtlCol="0">
            <a:normAutofit/>
          </a:bodyPr>
          <a:lstStyle/>
          <a:p>
            <a:pPr algn="{algn}"/>
            <a:r>
              <a:rPr lang="en-US" sz="{sz}"{bold}{italic} dirty="0">
                <a:solidFill><a:srgbClr val="{color}"/></a:solidFill>
                <a:latin typeface="{font}" panose="020F0502020204030204"/>
              </a:rPr>
              <a:t>{content}</a:t>
            </a:r>
          </a:p>
        </p:txBody>
      </p:sp>"#,
        id = el.id,
        x = x, y = y, cx = cx, cy = cy,
        sz = sz,
        bold = bold_attr,
        italic = italic_attr,
        algn = algn,
        color = color,
        font = escape_xml(&el.font_family),
        content = safe_content,
    )
}

fn timing_xml(slide: &PptxSlide) -> String {
    // Group elements by click_order
    let mut click_groups: HashMap<u32, Vec<&PptxElement>> = HashMap::new();
    for el in &slide.elements {
        if el.click_order > 0 {
            click_groups.entry(el.click_order).or_default().push(el);
        }
    }

    if click_groups.is_empty() {
        return String::new();
    }

    let mut max_click = *click_groups.keys().max().unwrap_or(&0);
    let mut tn_id = 2u32;
    let mut grp_id = 0u32;
    let mut click_pars = String::new();

    // Sort by click order
    let mut sorted_clicks: Vec<u32> = click_groups.keys().cloned().collect();
    sorted_clicks.sort();

    for click in sorted_clicks {
        let elements = &click_groups[&click];
        let mut anim_nodes = String::new();

        for el in elements.iter() {
            let ppt_anim = map_animation(&el.animation);
            let dur_emu = el.duration_ms * 1000; // PPT uses microseconds (hundredths of ms * 100)
            let cur_tn = tn_id;
            tn_id += 3;

            // Set visibility + animEffect
            anim_nodes.push_str(&format!(r#"                  <p:par>
                    <p:cTn id="{ctn}" presetID="{pid}" presetClass="{pcls}" presetSubtype="{psub}"
                           fill="hold" grpId="{gid}" nodeType="clickEffect">
                      <p:stCondLst><p:cond delay="0"/></p:stCondLst>
                      <p:childTnLst>
                        <p:set>
                          <p:cBhvr>
                            <p:cTn id="{ctn2}" dur="1" fill="hold"/>
                            <p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl>
                            <p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst>
                          </p:cBhvr>
                          <p:to><p:strVal val="visible"/></p:to>
                        </p:set>
                        <p:animEffect transition="in" filter="{filter}">
                          <p:cBhvr>
                            <p:cTn id="{ctn3}" dur="{dur}" decel="100000"/>
                            <p:tgtEl><p:spTgt spid="{spid}"/></p:tgtEl>
                          </p:cBhvr>
                        </p:animEffect>
                      </p:childTnLst>
                    </p:cTn>
                  </p:par>
"#,
                ctn = cur_tn,
                pid = ppt_anim.preset_id,
                pcls = ppt_anim.preset_class,
                psub = ppt_anim.preset_subtype,
                gid = grp_id,
                ctn2 = cur_tn + 1,
                ctn3 = cur_tn + 2,
                spid = el.id,
                filter = ppt_anim.filter,
                dur = dur_emu,
            ));
            grp_id += 1;
        }

        let outer_tn = tn_id;
        tn_id += 1;

        click_pars.push_str(&format!(r#"            <p:par>
              <p:cTn id="{otn}" fill="hold">
                <p:stCondLst><p:cond delay="indefinite"/></p:stCondLst>
                <p:childTnLst>
{anims}                </p:childTnLst>
              </p:cTn>
            </p:par>
"#,
            otn = outer_tn,
            anims = anim_nodes,
        ));
    }

    format!(r#"  <p:timing>
    <p:tnLst>
      <p:par>
        <p:cTn id="1" dur="indefinite" restart="whenNotActive" nodeType="tmRoot">
          <p:childTnLst>
{click_pars}          </p:childTnLst>
        </p:cTn>
      </p:par>
    </p:tnLst>
  </p:timing>"#,
        click_pars = click_pars,
    )
}

// ─── Static XML Templates ─────────────────────────────────────────────────────

fn content_types_xml(slide_count: usize) -> String {
    let slide_types: String = (1..=slide_count).map(|n| {
        format!(r#"  <Override PartName="/ppt/slides/slide{}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
"#, n)
    }).collect();

    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/package.core-properties+xml"/>
{slide_types}</Types>"#, slide_types = slide_types)
}

fn root_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#.to_string()
}

fn app_xml(title: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>Deckr</Application>
  <Company>Deckr AI</Company>
  <PresentationFormat>Widescreen</PresentationFormat>
</Properties>"#)
}

fn core_xml(title: &str) -> String {
    let safe_title = escape_xml(title);
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>{title}</dc:title>
  <dc:creator>Deckr AI</dc:creator>
</cp:coreProperties>"#, title = safe_title)
}

fn presentation_xml(slide_count: usize) -> String {
    let slide_ids: String = (1..=slide_count).map(|n| {
        format!(r#"    <p:sldId id="{}" r:id="rId{}"/>
"#, 255 + n, n + 3)
    }).collect();

    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                saveSubsetFonts="1">
  <p:sldMasterIdLst>
    <p:sldMasterId id="2147483648" r:id="rId1"/>
  </p:sldMasterIdLst>
  <p:sldSz cx="{sw}" cy="{sh}" type="screen16x9"/>
  <p:notesSz cx="6858000" cy="9144000"/>
  <p:sldIdLst>
{slide_ids}  </p:sldIdLst>
  <p:sldSz cx="{sw}" cy="{sh}"/>
</p:presentation>"#,
        sw = SLIDE_W, sh = SLIDE_H,
        slide_ids = slide_ids,
    )
}

fn presentation_rels_xml(slide_count: usize) -> String {
    let slide_rels: String = (1..=slide_count).map(|n| {
        format!(r#"  <Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>
"#, n + 3, n)
    }).collect();

    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/presProps" Target="presProps.xml"/>
{slide_rels}</Relationships>"#, slide_rels = slide_rels)
}

fn slide_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>"#.to_string()
}

fn slide_master_xml() -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <p:cSld>
    <p:bg><p:bgRef idx="1001"><a:schemeClr clrmap="bg1"/></p:bgRef></p:bg>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
  <p:sldLayoutIdLst>
    <p:sldLayoutId id="2147483649" r:id="rId1"/>
  </p:sldLayoutIdLst>
  <p:txStyles>
    <p:titleStyle><a:lvl1pPr algn="ctr"><a:defRPr lang="en-US"/></a:lvl1pPr></p:titleStyle>
    <p:bodyStyle><a:lvl1pPr><a:defRPr lang="en-US"/></a:lvl1pPr></p:bodyStyle>
    <p:otherStyle><a:lvl1pPr><a:defRPr lang="en-US"/></a:lvl1pPr></p:otherStyle>
  </p:txStyles>
</p:sldMaster>"#)
}

fn slide_master_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#.to_string()
}

fn slide_layout_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             type="blank" preserve="1">
  <p:cSld name="Blank">
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sldLayout>"#.to_string()
}

fn slide_layout_rels_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#.to_string()
}

fn theme_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Deckr Theme">
  <a:themeElements>
    <a:clrScheme name="Deckr">
      <a:dk1><a:srgbClr val="000000"/></a:dk1>
      <a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="1F2937"/></a:dk2>
      <a:lt2><a:srgbClr val="F3F4F6"/></a:lt2>
      <a:accent1><a:srgbClr val="6366F1"/></a:accent1>
      <a:accent2><a:srgbClr val="8B5CF6"/></a:accent2>
      <a:accent3><a:srgbClr val="EC4899"/></a:accent3>
      <a:accent4><a:srgbClr val="F59E0B"/></a:accent4>
      <a:accent5><a:srgbClr val="10B981"/></a:accent5>
      <a:accent6><a:srgbClr val="3B82F6"/></a:accent6>
      <a:hlink><a:srgbClr val="6366F1"/></a:hlink>
      <a:folHlink><a:srgbClr val="8B5CF6"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="Deckr">
      <a:majorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>
      <a:minorFont><a:latin typeface="Calibri"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont>
    </a:fontScheme>
    <a:fmtScheme name="Office">
      <a:fillStyleLst>
        <a:solidFill><a:schemeClr clrmap="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr clrmap="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr clrmap="phClr"/></a:solidFill>
      </a:fillStyleLst>
      <a:lnStyleLst>
        <a:ln w="6350"><a:solidFill><a:schemeClr clrmap="phClr"/></a:solidFill></a:ln>
        <a:ln w="12700"><a:solidFill><a:schemeClr clrmap="phClr"/></a:solidFill></a:ln>
        <a:ln w="19050"><a:solidFill><a:schemeClr clrmap="phClr"/></a:solidFill></a:ln>
      </a:lnStyleLst>
      <a:effectStyleLst>
        <a:effectStyle><a:effectLst/></a:effectStyle>
        <a:effectStyle><a:effectLst/></a:effectStyle>
        <a:effectStyle><a:effectLst/></a:effectStyle>
      </a:effectStyleLst>
      <a:bgFillStyleLst>
        <a:solidFill><a:schemeClr clrmap="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr clrmap="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr clrmap="phClr"/></a:solidFill>
      </a:bgFillStyleLst>
    </a:fmtScheme>
  </a:themeElements>
</a:theme>"#.to_string()
}

fn normalize_hex(hex: &str) -> String {
    let h = hex.trim_start_matches('#');
    if h.len() == 6 { h.to_uppercase() } else { "0F0F1F".to_string() }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&apos;")
}
