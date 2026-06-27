use crate::{
    ScanOptions, ToolCatalogEntry, format_preview, preview::category_label, preview_for_selection,
    scan_tools,
};
use floem::{Application, kurbo::Size, prelude::*, style::Style, window::WindowConfig};
use std::{collections::BTreeSet, env, path::PathBuf};

pub fn launch_ui() {
    let app = Application::new().window(
        |_| app_view(),
        Some(
            WindowConfig::default()
                .size(Size::new(980.0, 720.0))
                .min_size(Size::new(760.0, 520.0))
                .title("My AI Bag"),
        ),
    );
    app.run();
}

fn app_view() -> impl IntoView {
    let initial_project = env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .display()
        .to_string();
    let initial_home = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .display()
        .to_string();

    let project_root = RwSignal::new(initial_project);
    let search_root = RwSignal::new(initial_home);
    let report = RwSignal::new(scan_from_inputs(
        &search_root.get_untracked(),
        &project_root.get_untracked(),
    ));
    let selected_tools = RwSignal::new(default_selected(&report.get_untracked()));
    let preview_text = RwSignal::new(build_preview_text(
        &report.get_untracked(),
        &selected_tools.get_untracked(),
        &[],
    ));
    let candidate_key = RwSignal::new(String::new());
    let candidate_name = RwSignal::new(String::new());
    let candidates = RwSignal::new(Vec::<String>::new());

    let refresh = move || {
        let next_report =
            scan_from_inputs(&search_root.get_untracked(), &project_root.get_untracked());
        let next_selected = default_selected(&next_report);
        report.set(next_report.clone());
        selected_tools.set(next_selected.clone());
        preview_text.set(build_preview_text(
            &next_report,
            &next_selected,
            &candidates.get_untracked(),
        ));
    };

    let pack_preview = move || {
        preview_text.set(build_preview_text(
            &report.get_untracked(),
            &selected_tools.get_untracked(),
            &candidates.get_untracked(),
        ));
    };

    let add_candidate = move || {
        let key = candidate_key.get_untracked().trim().to_string();
        let name = candidate_name.get_untracked().trim().to_string();
        if key.is_empty() && name.is_empty() {
            return;
        }
        let label = if name.is_empty() {
            key.clone()
        } else if key.is_empty() {
            name.clone()
        } else {
            format!("{key} - {name}")
        };
        candidates.update(|items| {
            if !items.contains(&label) {
                items.push(label);
            }
        });
        candidate_key.set(String::new());
        candidate_name.set(String::new());
        pack_preview();
    };

    Stack::vertical((
        header(),
        Stack::horizontal((
            Stack::vertical((
                field("Project root", project_root, "Current project folder"),
                field("Search/home root", search_root, "Usually your home folder"),
                Stack::horizontal((
                    Button::new("Scan").action(refresh).style(button_style),
                    Button::new("Pack Bag")
                        .action(pack_preview)
                        .style(primary_button_style),
                ))
                .style(|s| s.gap(10.0)),
                tools_panel(report, selected_tools),
                candidate_panel(candidate_key, candidate_name, candidates, add_candidate),
            ))
            .style(|s| s.width_pct(48.0).min_height(0.0).gap(12.0)),
            preview_panel(preview_text)
                .style(|s| s.flex_basis(0.0).min_size(0.0, 0.0).flex_grow(1.0)),
        ))
        .style(|s| {
            s.gap(18.0)
                .width_full()
                .flex_basis(0.0)
                .min_height(0.0)
                .flex_grow(1.0)
        }),
    ))
    .style(|s| {
        s.size_full()
            .padding(20.0)
            .gap(16.0)
            .background(Color::from_rgb8(18, 22, 30))
            .color(Color::from_rgb8(232, 238, 247))
    })
}

fn header() -> impl IntoView {
    Stack::vertical((
        "My AI Bag".style(|s| s.font_size(30.0)),
        "Your AI coding bag is packed and ready."
            .style(|s| s.color(Color::from_rgb8(160, 177, 197)).font_size(14.0)),
    ))
    .style(|s| s.gap(4.0))
}

fn field(
    label_text: &'static str,
    signal: RwSignal<String>,
    placeholder: &'static str,
) -> impl IntoView {
    Stack::vertical((
        label_text.style(|s| s.font_size(12.0).color(Color::from_rgb8(160, 177, 197))),
        TextInput::new(signal).placeholder(placeholder).style(|s| {
            s.width_full()
                .padding(9.0)
                .border(1.0)
                .border_radius(6.0)
                .border_color(Color::from_rgb8(72, 88, 112))
                .background(Color::from_rgb8(27, 33, 44))
                .color(Color::from_rgb8(238, 242, 247))
        }),
    ))
    .style(|s| s.gap(5.0).width_full())
}

fn tools_panel(
    report: RwSignal<crate::ScanReport>,
    selected: RwSignal<BTreeSet<String>>,
) -> impl IntoView {
    Stack::vertical((
        "Supported tools".style(|s| s.font_size(16.0)),
        dyn_stack(
            move || report.get().tools,
            |tool| tool.key.clone(),
            move |tool| tool_row(tool, selected),
        )
        .style(|s| s.flex_col().gap(6.0))
        .scroll()
        .style(|s| {
            s.height(310.0)
                .border(1.0)
                .border_color(Color::from_rgb8(52, 65, 86))
                .border_radius(8.0)
                .background(Color::from_rgb8(13, 17, 24))
                .padding(8.0)
        }),
    ))
    .style(|s| s.gap(8.0).width_full())
}

fn tool_row(tool: crate::ToolScan, selected: RwSignal<BTreeSet<String>>) -> impl IntoView {
    let key_for_check = tool.key.clone();
    let key_for_update = tool.key.clone();
    let status = if tool.detected {
        "detected"
    } else {
        "not found"
    };
    let found_summary = tool
        .found
        .iter()
        .map(|item| category_label(item.category))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    let detail = if found_summary.is_empty() {
        "no folders yet".to_string()
    } else {
        format!("found {found_summary}")
    };

    Stack::vertical((
        Checkbox::labeled(
            move || selected.get().contains(&key_for_check),
            move || format!("{} ({status})", tool.display_name),
        )
        .on_update(move |checked| {
            selected.update(|items| {
                if checked {
                    items.insert(key_for_update.clone());
                } else {
                    items.remove(&key_for_update);
                }
            });
        })
        .style(|s| s.gap(8.0)),
        detail.style(|s| s.font_size(11.0).color(Color::from_rgb8(151, 163, 183))),
    ))
    .style(|s| {
        s.gap(3.0)
            .padding(8.0)
            .border_radius(6.0)
            .background(Color::from_rgb8(25, 31, 42))
    })
}

fn candidate_panel(
    candidate_key: RwSignal<String>,
    candidate_name: RwSignal<String>,
    candidates: RwSignal<Vec<String>>,
    add_candidate: impl Fn() + Copy + 'static,
) -> impl IntoView {
    Stack::vertical((
        "Add tool candidate".style(|s| s.font_size(16.0)),
        Stack::horizontal((
            TextInput::new(candidate_key)
                .placeholder("tool_key")
                .style(|s| {
                    s.width_pct(42.0)
                        .padding(8.0)
                        .border(1.0)
                        .border_radius(6.0)
                }),
            TextInput::new(candidate_name)
                .placeholder("Display name")
                .style(|s| s.flex_grow(1.0).padding(8.0).border(1.0).border_radius(6.0)),
            Button::new("Add").action(add_candidate).style(button_style),
        ))
        .style(|s| s.gap(8.0).width_full()),
        Label::derived(move || {
            let items = candidates.get();
            if items.is_empty() {
                "No candidates added yet.".to_string()
            } else {
                format!("Candidate queue: {}", items.join(", "))
            }
        })
        .style(|s| s.font_size(12.0).color(Color::from_rgb8(160, 177, 197))),
    ))
    .style(|s| s.gap(8.0).width_full())
}

fn preview_panel(preview_text: RwSignal<String>) -> impl IntoView {
    Stack::vertical((
        "Pack preview".style(|s| s.font_size(16.0)),
        Label::derived(move || preview_text.get())
            .style(|s| {
                s.font_family("monospace".to_string())
                    .font_size(12.0)
                    .line_height(1.35)
                    .color(Color::from_rgb8(221, 229, 239))
            })
            .scroll()
            .style(|s| {
                s.width_full()
                    .flex_basis(0.0)
                    .min_size(0.0, 0.0)
                    .flex_grow(1.0)
                    .border(1.0)
                    .border_color(Color::from_rgb8(52, 65, 86))
                    .border_radius(8.0)
                    .background(Color::from_rgb8(10, 13, 18))
                    .padding(12.0)
            }),
    ))
    .style(|s| {
        s.flex_col()
            .gap(8.0)
            .width_full()
            .height_full()
            .min_size(0.0, 0.0)
    })
}

fn build_preview_text(
    report: &crate::ScanReport,
    selected: &BTreeSet<String>,
    candidates: &[String],
) -> String {
    let selected_keys = selected.iter().cloned().collect::<Vec<_>>();
    let preview = preview_for_selection(report, &selected_keys);
    let mut text = format_preview(report, &preview);
    if !candidates.is_empty() {
        text.push_str("\n\nHuman review candidates:\n");
        for candidate in candidates {
            text.push_str("- ");
            text.push_str(candidate);
            text.push('\n');
        }
    }
    text
}

fn scan_from_inputs(search_root: &str, project_root: &str) -> crate::ScanReport {
    let home_dir = if search_root.trim().is_empty() {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    } else {
        PathBuf::from(search_root.trim())
    };
    let project_root = if project_root.trim().is_empty() {
        env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(project_root.trim())
    };

    scan_tools(&ScanOptions {
        home_dir,
        project_root,
        include_home: true,
        include_project: true,
        catalog: crate::builtin_tools(),
    })
}

fn default_selected(report: &crate::ScanReport) -> BTreeSet<String> {
    report
        .tools
        .iter()
        .filter(|tool| tool.detected)
        .map(|tool| tool.key.clone())
        .collect()
}

fn button_style(s: Style) -> Style {
    s.padding_horiz(14.0)
        .padding_vert(8.0)
        .border_radius(6.0)
        .background(Color::from_rgb8(39, 50, 68))
        .color(Color::from_rgb8(238, 242, 247))
}

fn primary_button_style(s: Style) -> Style {
    s.padding_horiz(14.0)
        .padding_vert(8.0)
        .border_radius(6.0)
        .background(Color::from_rgb8(0, 168, 168))
        .color(Color::from_rgb8(5, 12, 18))
}

#[allow(dead_code)]
fn custom_entry_from_candidate(key: &'static str, name: &'static str) -> ToolCatalogEntry {
    ToolCatalogEntry {
        key: key.to_string(),
        display_name: name.to_string(),
        global_skills_dir: ".agents/skills".to_string(),
        project_skills_dir: Some(".agents/skills".to_string()),
        detected_if_exists: ".agents".to_string(),
        home_roots: Vec::new(),
        project_roots: Vec::new(),
    }
}
