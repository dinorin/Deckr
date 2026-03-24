/// Maps a CSS animation class name to PowerPoint preset animation attributes.
/// Returns (preset_id, preset_class, preset_subtype, filter, subtype_str)
pub struct PptAnimation {
    pub preset_id: u32,
    pub preset_class: &'static str, // "entr" | "emph" | "exit"
    pub preset_subtype: u32,
    pub filter: &'static str,       // for animEffect
    pub filter_subtype: &'static str,
}

pub fn map_animation(css_anim: &str) -> PptAnimation {
    match css_anim {
        "appear" => PptAnimation {
            preset_id: 1,
            preset_class: "entr",
            preset_subtype: 0,
            filter: "appear",
            filter_subtype: "",
        },
        "fade-in" | "fade" => PptAnimation {
            preset_id: 10,
            preset_class: "entr",
            preset_subtype: 0,
            filter: "fade",
            filter_subtype: "",
        },
        "fly-in-bottom" => PptAnimation {
            preset_id: 2,
            preset_class: "entr",
            preset_subtype: 2,
            filter: "fly",
            filter_subtype: "fromBottom",
        },
        "fly-in-top" => PptAnimation {
            preset_id: 2,
            preset_class: "entr",
            preset_subtype: 1,
            filter: "fly",
            filter_subtype: "fromTop",
        },
        "fly-in-left" => PptAnimation {
            preset_id: 2,
            preset_class: "entr",
            preset_subtype: 8,
            filter: "fly",
            filter_subtype: "fromLeft",
        },
        "fly-in-right" => PptAnimation {
            preset_id: 2,
            preset_class: "entr",
            preset_subtype: 4,
            filter: "fly",
            filter_subtype: "fromRight",
        },
        "float-in" => PptAnimation {
            preset_id: 7,
            preset_class: "entr",
            preset_subtype: 0,
            filter: "fly",
            filter_subtype: "fromBottom",
        },
        "zoom-in" => PptAnimation {
            preset_id: 22,
            preset_class: "entr",
            preset_subtype: 0,
            filter: "zoom",
            filter_subtype: "in",
        },
        "bounce-in" => PptAnimation {
            preset_id: 26,
            preset_class: "entr",
            preset_subtype: 0,
            filter: "bounce",
            filter_subtype: "",
        },
        "wipe-left" => PptAnimation {
            preset_id: 35,
            preset_class: "entr",
            preset_subtype: 8,
            filter: "wipe",
            filter_subtype: "left",
        },
        "wipe-right" => PptAnimation {
            preset_id: 35,
            preset_class: "entr",
            preset_subtype: 4,
            filter: "wipe",
            filter_subtype: "right",
        },
        "split" => PptAnimation {
            preset_id: 34,
            preset_class: "entr",
            preset_subtype: 0,
            filter: "split",
            filter_subtype: "",
        },
        "swivel" => PptAnimation {
            preset_id: 28,
            preset_class: "entr",
            preset_subtype: 0,
            filter: "swivel",
            filter_subtype: "horizontal",
        },
        // Default fallback
        _ => PptAnimation {
            preset_id: 10,
            preset_class: "entr",
            preset_subtype: 0,
            filter: "fade",
            filter_subtype: "",
        },
    }
}
