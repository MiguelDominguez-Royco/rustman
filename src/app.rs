use crate::http_client::{
    generate_code_snippet, import_collections_from_json, interpolate_vars, normalize_url,
    parse_curl_command, send_http_request,
};
use crate::models::{
    AppStorage, AssertionKind, AuthConfig, BodyType, CodeLang, Collection, Environment,
    HistoryEntry, HttpMethod, HttpRequestItem, HttpResponseData, KeyValue, MultipartField,
    MultipartKind, ResponseFormat, TestAssertion, ThemeMode, VarExtraction,
};
use chrono::Local;
use eframe::egui;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use tokio::runtime::Runtime;
use uuid::Uuid;

#[derive(PartialEq, Eq, Clone, Copy)]
enum RequestTab {
    Params,
    Auth,
    Headers,
    Body,
    TestsAndVars,
    CodeGen,
    Settings,
    EnvPreview,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum ResponseTab {
    Pretty,
    Raw,
    Preview,
    Headers,
    Cookies,
    Tests,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum SidebarTab {
    Collections,
    History,
    Environments,
    Cookies,
}

pub struct WorkspaceTab {
    pub tab_id: Uuid,
    pub request: HttpRequestItem,
    pub response: Option<HttpResponseData>,
    pub is_loading: bool,
    pub dirty: bool,
}

impl WorkspaceTab {
    pub fn new(request: HttpRequestItem) -> Self {
        Self {
            tab_id: Uuid::new_v4(),
            request,
            response: None,
            is_loading: false,
            dirty: false,
        }
    }
}

pub struct RustmanApp {
    storage: AppStorage,
    tabs: Vec<WorkspaceTab>,
    active_tab_idx: usize,

    // Pestañas de sección activas
    req_tab: RequestTab,
    resp_tab: ResponseTab,
    sidebar_tab: SidebarTab,

    // Estado de Generador de Código / Importador cURL
    selected_code_lang: CodeLang,
    curl_import_input: String,

    // Estado de búsqueda en la respuesta
    response_search_query: String,
    response_filter_matching_lines: bool,
    focus_url_next_frame: bool,
    focus_search_next_frame: bool,

    // Modales / Estado auxiliar de UI
    new_collection_name: String,
    selected_collection_idx: usize,
    new_env_name: String,
    status_message: Option<String>,

    // Runtime asíncrono y canal de comunicación (tab_id, respuesta)
    rt: Runtime,
    tx: Sender<(Uuid, HttpResponseData)>,
    rx: Receiver<(Uuid, HttpResponseData)>,
}

// Paleta oficial Catppuccin (Mocha, Macchiato, Frappé, Latte) con alto contraste
struct ThemePalette {
    pub base: egui::Color32,      // Fondo principal de paneles
    pub mantle: egui::Color32,    // Barra superior / panel lateral
    pub crust: egui::Color32,     // Inputs, editores de código, fondo profundo
    pub surface0: egui::Color32,  // Botones en reposo / tarjetas
    pub surface1: egui::Color32,  // Hover / Bordes suaves
    pub text: egui::Color32,      // Texto principal de alto contraste
    pub subtext: egui::Color32,   // Etiquetas secundarias
    pub overlay: egui::Color32,   // Placeholders / Notas
    pub blue: egui::Color32,
    pub sapphire: egui::Color32,
    pub mauve: egui::Color32,
    pub green: egui::Color32,
    pub peach: egui::Color32,
    pub red: egui::Color32,
    pub active_bg: egui::Color32, // Fondo de pestañas/elementos seleccionados
    pub on_accent: egui::Color32, // Texto sobre botones de acento
}

impl ThemePalette {
    pub fn for_mode(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::CatppuccinMocha => Self {
                base: egui::Color32::from_rgb(30, 30, 46),        // #1e1e2e
                mantle: egui::Color32::from_rgb(24, 24, 37),      // #181825
                crust: egui::Color32::from_rgb(17, 17, 27),       // #11111b
                surface0: egui::Color32::from_rgb(49, 50, 68),    // #313244
                surface1: egui::Color32::from_rgb(69, 71, 90),    // #45475a
                text: egui::Color32::from_rgb(238, 242, 255),     // Blanco lavanda ultra nítido
                subtext: egui::Color32::from_rgb(205, 214, 244),  // #cdd6f4
                overlay: egui::Color32::from_rgb(147, 153, 178),  // #9399b2
                blue: egui::Color32::from_rgb(137, 180, 250),     // #89b4fa
                sapphire: egui::Color32::from_rgb(116, 199, 236), // #74c7ec
                mauve: egui::Color32::from_rgb(203, 166, 247),    // #cba6f7
                green: egui::Color32::from_rgb(166, 227, 161),    // #a6e3a1
                peach: egui::Color32::from_rgb(250, 179, 135),    // #fab387
                red: egui::Color32::from_rgb(243, 139, 168),      // #f38ba8
                active_bg: egui::Color32::from_rgb(76, 86, 145),
                on_accent: egui::Color32::from_rgb(17, 17, 27),   // #11111b
            },
            ThemeMode::CatppuccinMacchiato => Self {
                base: egui::Color32::from_rgb(36, 39, 58),        // #24273a
                mantle: egui::Color32::from_rgb(30, 32, 48),      // #1e2030
                crust: egui::Color32::from_rgb(24, 25, 38),       // #181926
                surface0: egui::Color32::from_rgb(54, 58, 79),    // #363a4f
                surface1: egui::Color32::from_rgb(73, 77, 100),   // #494d64
                text: egui::Color32::from_rgb(238, 242, 255),
                subtext: egui::Color32::from_rgb(202, 211, 245),  // #cad3f5
                overlay: egui::Color32::from_rgb(147, 154, 183),  // #939ab7
                blue: egui::Color32::from_rgb(138, 173, 244),     // #8aadf4
                sapphire: egui::Color32::from_rgb(125, 196, 228), // #7dc4e4
                mauve: egui::Color32::from_rgb(198, 160, 246),    // #c6a0f6
                green: egui::Color32::from_rgb(166, 218, 149),    // #a6da95
                peach: egui::Color32::from_rgb(245, 169, 127),    // #f5a97f
                red: egui::Color32::from_rgb(237, 135, 150),      // #ed8796
                active_bg: egui::Color32::from_rgb(80, 92, 155),
                on_accent: egui::Color32::from_rgb(24, 25, 38),
            },
            ThemeMode::CatppuccinFrappe => Self {
                base: egui::Color32::from_rgb(48, 52, 70),        // #303446
                mantle: egui::Color32::from_rgb(41, 44, 60),      // #292c3c
                crust: egui::Color32::from_rgb(35, 38, 52),       // #232634
                surface0: egui::Color32::from_rgb(65, 69, 89),    // #414559
                surface1: egui::Color32::from_rgb(81, 87, 109),   // #51576d
                text: egui::Color32::from_rgb(242, 245, 255),
                subtext: egui::Color32::from_rgb(198, 208, 245),  // #c6d0f5
                overlay: egui::Color32::from_rgb(148, 156, 187),  // #949cbb
                blue: egui::Color32::from_rgb(140, 170, 238),     // #8caaee
                sapphire: egui::Color32::from_rgb(133, 193, 220), // #85c1dc
                mauve: egui::Color32::from_rgb(202, 158, 230),    // #ca9ee6
                green: egui::Color32::from_rgb(166, 209, 137),    // #a6d189
                peach: egui::Color32::from_rgb(239, 159, 118),    // #ef9f76
                red: egui::Color32::from_rgb(231, 130, 132),      // #e78284
                active_bg: egui::Color32::from_rgb(88, 100, 165),
                on_accent: egui::Color32::from_rgb(35, 38, 52),
            },
            ThemeMode::CatppuccinLatte => Self {
                base: egui::Color32::from_rgb(239, 241, 245),     // #eff1f5
                mantle: egui::Color32::from_rgb(230, 233, 239),   // #e6e9ef
                crust: egui::Color32::from_rgb(255, 255, 255),    // #ffffff
                surface0: egui::Color32::from_rgb(204, 208, 218), // #ccd0da
                surface1: egui::Color32::from_rgb(188, 192, 204), // #bcc0cc
                text: egui::Color32::from_rgb(25, 28, 42),        // Negro Carbón profundo (#191c2a)
                subtext: egui::Color32::from_rgb(55, 60, 82),
                overlay: egui::Color32::from_rgb(108, 111, 133),  // #6c6f85
                blue: egui::Color32::from_rgb(30, 102, 245),      // #1e66f5
                sapphire: egui::Color32::from_rgb(32, 159, 181),  // #209fb5
                mauve: egui::Color32::from_rgb(136, 57, 239),     // #8839ef
                green: egui::Color32::from_rgb(64, 160, 43),      // #40a02b
                peach: egui::Color32::from_rgb(254, 100, 11),     // #fe640b
                red: egui::Color32::from_rgb(210, 15, 57),        // #d20f39
                active_bg: egui::Color32::from_rgb(30, 102, 245), // #1e66f5
                on_accent: egui::Color32::WHITE,
            },
        }
    }
}

const COMMON_HEADERS: &[(&str, &[&str])] = &[
    (
        "Accept",
        &[
            "application/json",
            "*/*",
            "text/html",
            "application/xml",
            "text/plain",
            "multipart/form-data",
        ],
    ),
    (
        "Content-Type",
        &[
            "application/json",
            "application/x-www-form-urlencoded",
            "multipart/form-data",
            "text/plain; charset=utf-8",
            "application/xml",
            "application/octet-stream",
        ],
    ),
    (
        "Authorization",
        &["Bearer {{api_token}}", "Basic ", "Digest "],
    ),
    (
        "Cache-Control",
        &[
            "no-cache",
            "no-store",
            "max-age=0",
            "max-age=3600",
            "public",
            "private",
        ],
    ),
    (
        "Accept-Encoding",
        &["gzip, deflate, br", "gzip, deflate", "identity"],
    ),
    (
        "Accept-Language",
        &["es-MX,es;q=0.9,en;q=0.8", "en-US,en;q=0.9", "*"],
    ),
    ("Connection", &["keep-alive", "close"]),
    (
        "User-Agent",
        &[
            "Rustman/0.2.0",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
            "PostmanRuntime/7.36.0",
        ],
    ),
    ("X-Requested-With", &["XMLHttpRequest"]),
    ("X-API-Key", &["{{api_token}}"]),
    (
        "Access-Control-Allow-Origin",
        &["*", "https://ejemplo.com"],
    ),
    (
        "Access-Control-Allow-Methods",
        &["GET, POST, PUT, DELETE, PATCH, OPTIONS"],
    ),
];

impl RustmanApp {
    fn col_header(ui: &mut egui::Ui, text: &str, color: egui::Color32) {
        ui.label(egui::RichText::new(text).strong().color(color));
    }

    fn header_values_for_key(key: &str) -> &'static [&'static str] {
        for (k, vals) in COMMON_HEADERS {
            if k.eq_ignore_ascii_case(key.trim()) {
                return vals;
            }
        }
        &[
            "application/json",
            "*/*",
            "no-cache",
            "keep-alive",
            "utf-8",
            "true",
            "false",
        ]
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let storage = Self::load_storage_from_disk().unwrap_or_default();
        Self::apply_theme(&cc.egui_ctx, storage.theme);

        let (tx, rx) = channel();
        let rt = Runtime::new().expect("No se pudo iniciar el runtime de Tokio");

        Self {
            storage,
            tabs: vec![WorkspaceTab::new(HttpRequestItem::default())],
            active_tab_idx: 0,
            req_tab: RequestTab::Params,
            resp_tab: ResponseTab::Pretty,
            sidebar_tab: SidebarTab::Collections,
            selected_code_lang: CodeLang::Curl,
            curl_import_input: String::new(),
            response_search_query: String::new(),
            response_filter_matching_lines: false,
            focus_url_next_frame: false,
            focus_search_next_frame: false,
            new_collection_name: String::new(),
            selected_collection_idx: 0,
            new_env_name: String::new(),
            status_message: None,
            rt,
            tx,
            rx,
        }
    }

    fn open_or_focus_request(&mut self, req: HttpRequestItem) {
        if let Some(existing_idx) = self.tabs.iter().position(|t| t.request.id == req.id) {
            self.active_tab_idx = existing_idx;
            return;
        }
        // Si la pestaña actual no tiene cambios y no está cargando, o podemos abrir una nueva pestaña
        if self.tabs.len() == 1
            && !self.tabs[0].dirty
            && self.tabs[0].response.is_none()
            && self.tabs[0].request.name == "Nueva Petición"
        {
            self.tabs[0] = WorkspaceTab::new(req);
            self.active_tab_idx = 0;
        } else {
            self.tabs.push(WorkspaceTab::new(req));
            self.active_tab_idx = self.tabs.len() - 1;
        }
    }

    fn save_current_request_to_collection(&mut self) {
        if self.storage.collections.is_empty() {
            self.storage.collections.push(Collection {
                id: Uuid::new_v4(),
                name: "Mi Colección".to_string(),
                requests: Vec::new(),
            });
            self.selected_collection_idx = 0;
        }
        if self.selected_collection_idx >= self.storage.collections.len() {
            self.selected_collection_idx = 0;
        }

        let current_req = self.tabs[self.active_tab_idx].request.clone();

        // Verificar si ya existe por ID en alguna colección para actualizarla in-place
        for col in &mut self.storage.collections {
            if let Some(existing) = col.requests.iter_mut().find(|r| r.id == current_req.id) {
                *existing = current_req;
                self.tabs[self.active_tab_idx].dirty = false;
                self.save_storage_to_disk();
                self.status_message =
                    Some("💾 Petición actualizada en la colección (Ctrl+S).".to_string());
                return;
            }
        }

        // Si no existía, agregarla a la colección seleccionada
        self.storage.collections[self.selected_collection_idx]
            .requests
            .push(current_req);
        self.tabs[self.active_tab_idx].dirty = false;
        self.save_storage_to_disk();
        self.status_message = Some("💾 Petición guardada en la colección (Ctrl+S).".to_string());
    }

    /// Sincroniza la tabla de Query Params hacia la cadena de la URL (al estilo Postman: `http://127.0.0.1?hola=hola`)
    fn sync_params_to_url(url: &mut String, params: &[KeyValue]) {
        let base = url.split('?').next().unwrap_or("").to_string();
        let active_pairs: Vec<String> = params
            .iter()
            .filter(|p| p.enabled && (!p.key.is_empty() || !p.value.is_empty()))
            .map(|p| {
                if p.value.is_empty() {
                    p.key.clone()
                } else {
                    format!("{}={}", p.key, p.value)
                }
            })
            .collect();

        if active_pairs.is_empty() {
            *url = base;
        } else {
            *url = format!("{}?{}", base, active_pairs.join("&"));
        }
    }

    /// Sincroniza la cadena de la URL (cuando el usuario escribe `?clave=valor` arriba) hacia la tabla de Query Params
    fn sync_url_to_params(url: &str, params: &mut Vec<KeyValue>) {
        let disabled_params: Vec<KeyValue> =
            params.iter().filter(|p| !p.enabled).cloned().collect();

        let mut new_params = Vec::new();
        if let Some((_, query)) = url.split_once('?') {
            if !query.is_empty() {
                for pair in query.split('&') {
                    if pair.is_empty() {
                        continue;
                    }
                    let (k, v) = match pair.split_once('=') {
                        Some((k, v)) => (k.to_string(), v.to_string()),
                        None => (pair.to_string(), String::new()),
                    };
                    new_params.push(KeyValue {
                        enabled: true,
                        key: k,
                        value: v,
                        description: String::new(),
                    });
                }
            }
        }

        new_params.extend(disabled_params);
        if new_params.is_empty() {
            new_params.push(KeyValue::empty());
        }
        *params = new_params;
    }

    /// Renderiza código JSON, XML, HTML o texto con resaltado de sintaxis Catppuccin y resaltado de búsqueda
    fn render_syntax_highlighted_code(
        ui: &mut egui::Ui,
        code: &str,
        format: ResponseFormat,
        search_query: &str,
        filter_matching: bool,
        palette: &ThemePalette,
    ) {
        let query_lower = search_query.trim().to_lowercase();
        let font_id = egui::FontId::monospace(13.0);

        let mut job = egui::text::LayoutJob::default();
        let highlight_bg = if palette.on_accent == egui::Color32::WHITE {
            egui::Color32::from_rgb(255, 230, 128)
        } else {
            egui::Color32::from_rgb(95, 75, 40)
        };

        for (line_idx, line) in code.lines().enumerate() {
            if filter_matching
                && !query_lower.is_empty()
                && !line.to_lowercase().contains(&query_lower)
            {
                continue;
            }

            // Número de línea
            job.append(
                &format!("{:>4} │ ", line_idx + 1),
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: palette.overlay,
                    ..Default::default()
                },
            );

            // Si hay búsqueda activa y coincide en esta línea, resaltar ocurrencias
            if !query_lower.is_empty() && line.to_lowercase().contains(&query_lower) {
                let lower_line = line.to_lowercase();
                let mut last_pos = 0;
                while let Some(rel_idx) = lower_line[last_pos..].find(&query_lower) {
                    let match_start = last_pos + rel_idx;
                    let match_end = (match_start + query_lower.len()).min(line.len());
                    if line.is_char_boundary(last_pos) && line.is_char_boundary(match_start) {
                        Self::append_syntax_segment(
                            &mut job,
                            &line[last_pos..match_start],
                            format,
                            &font_id,
                            palette,
                        );
                    }
                    if line.is_char_boundary(match_start) && line.is_char_boundary(match_end) {
                        job.append(
                            &line[match_start..match_end],
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: palette.text,
                                background: highlight_bg,
                                ..Default::default()
                            },
                        );
                    }
                    last_pos = match_end;
                    if last_pos >= line.len() {
                        break;
                    }
                }
                if last_pos < line.len() && line.is_char_boundary(last_pos) {
                    Self::append_syntax_segment(
                        &mut job,
                        &line[last_pos..],
                        format,
                        &font_id,
                        palette,
                    );
                }
            } else {
                Self::append_syntax_segment(&mut job, line, format, &font_id, palette);
            }

            job.append(
                "\n",
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: palette.text,
                    ..Default::default()
                },
            );
        }

        egui::Frame::none()
            .fill(palette.crust)
            .inner_margin(egui::Margin::same(10.0))
            .rounding(egui::Rounding::same(6.0))
            .stroke(egui::Stroke::new(1.0_f32, palette.surface1))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.add(egui::Label::new(job).selectable(true));
            });
    }

    fn append_syntax_segment(
        job: &mut egui::text::LayoutJob,
        line: &str,
        format: ResponseFormat,
        font_id: &egui::FontId,
        palette: &ThemePalette,
    ) {
        match format {
            ResponseFormat::Json => {
                // Colorear claves JSON ("key":) vs valores ("string", números, booleanos)
                if let Some((left, right)) = line.split_once(':') {
                    if left.trim_start().starts_with('"') {
                        job.append(
                            left,
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: palette.blue,
                                ..Default::default()
                            },
                        );
                        job.append(
                            ":",
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: palette.mauve,
                                ..Default::default()
                            },
                        );
                        let trimmed_r = right.trim();
                        let val_color = if trimmed_r.starts_with('"') {
                            palette.green
                        } else if trimmed_r.starts_with("true")
                            || trimmed_r.starts_with("false")
                            || trimmed_r.starts_with("null")
                        {
                            palette.mauve
                        } else if trimmed_r
                            .chars()
                            .next()
                            .map(|c| c.is_ascii_digit() || c == '-')
                            .unwrap_or(false)
                        {
                            palette.peach
                        } else {
                            palette.text
                        };
                        job.append(
                            right,
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: val_color,
                                ..Default::default()
                            },
                        );
                        return;
                    }
                }
                let trimmed = line.trim();
                let color = if trimmed.starts_with('"') {
                    palette.green
                } else if trimmed == "{"
                    || trimmed == "}"
                    || trimmed == "},"
                    || trimmed == "["
                    || trimmed == "]"
                    || trimmed == "],"
                {
                    palette.mauve
                } else {
                    palette.text
                };
                job.append(
                    line,
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color,
                        ..Default::default()
                    },
                );
            }
            ResponseFormat::Xml | ResponseFormat::Html => {
                let trimmed = line.trim();
                let color = if trimmed.starts_with("<?") || trimmed.starts_with("<!") {
                    palette.peach
                } else if trimmed.starts_with("</") {
                    palette.mauve
                } else if trimmed.starts_with('<') {
                    palette.blue
                } else {
                    palette.text
                };
                job.append(
                    line,
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color,
                        ..Default::default()
                    },
                );
            }
            ResponseFormat::Text => {
                job.append(
                    line,
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: palette.text,
                        ..Default::default()
                    },
                );
            }
        }
    }

    /// Resaltador de sintaxis JSON en tiempo real para `egui::TextEdit::multiline` (preserva 1:1 cada byte para el cursor)
    fn build_json_editor_layout_job(
        text: &str,
        palette: &ThemePalette,
        wrap_width: f32,
    ) -> egui::text::LayoutJob {
        let mut job = egui::text::LayoutJob::default();
        job.wrap.max_width = wrap_width;
        let font_id = egui::FontId::monospace(13.5);

        let bytes = text.as_bytes();
        let len = bytes.len();
        let mut i = 0;

        while i < len {
            let b = bytes[i];

            // 1. Espacios en blanco y saltos de línea
            if b.is_ascii_whitespace() {
                let start = i;
                while i < len && bytes[i].is_ascii_whitespace() {
                    i += 1;
                }
                job.append(
                    &text[start..i],
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: palette.text,
                        ..Default::default()
                    },
                );
                continue;
            }

            // 2. Cadenas de texto entre comillas ("key" o "value")
            if b == b'"' {
                let start = i;
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' {
                        i = (i + 2).min(len);
                    } else if bytes[i] == b'"' {
                        i += 1;
                        break;
                    } else {
                        let ch = text[i..].chars().next().unwrap_or(' ');
                        i += ch.len_utf8();
                    }
                }
                let str_slice = &text[start..i];

                // Mirar hacia adelante si el siguiente carácter no-blanco es ':' -> es una Clave JSON
                let mut lookahead = i;
                while lookahead < len && bytes[lookahead].is_ascii_whitespace() {
                    lookahead += 1;
                }
                let is_key = lookahead < len && bytes[lookahead] == b':';
                let base_color = if is_key { palette.blue } else { palette.green };

                // Si contiene {{variable}}, resaltar la variable en color durazno (peach)
                let mut rem = str_slice;
                while let Some(var_start) = rem.find("{{") {
                    if var_start > 0 {
                        job.append(
                            &rem[..var_start],
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: base_color,
                                ..Default::default()
                            },
                        );
                    }
                    let after_open = &rem[var_start..];
                    if let Some(var_end_rel) = after_open.find("}}") {
                        let var_end = var_end_rel + 2;
                        job.append(
                            &after_open[..var_end],
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: palette.peach,
                                ..Default::default()
                            },
                        );
                        rem = &after_open[var_end..];
                    } else {
                        job.append(
                            after_open,
                            0.0,
                            egui::TextFormat {
                                font_id: font_id.clone(),
                                color: palette.peach,
                                ..Default::default()
                            },
                        );
                        rem = "";
                        break;
                    }
                }
                if !rem.is_empty() {
                    job.append(
                        rem,
                        0.0,
                        egui::TextFormat {
                            font_id: font_id.clone(),
                            color: base_color,
                            ..Default::default()
                        },
                    );
                }
                continue;
            }

            // 3. Variables de entorno {{var}} fuera de comillas
            if text[i..].starts_with("{{") {
                let start = i;
                if let Some(end_rel) = text[i..].find("}}") {
                    i += end_rel + 2;
                } else {
                    i = len;
                }
                job.append(
                    &text[start..i],
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: palette.peach,
                        ..Default::default()
                    },
                );
                continue;
            }

            // 4. Llaves y corchetes estructurales
            if matches!(b, b'{' | b'}' | b'[' | b']') {
                let start = i;
                i += 1;
                job.append(
                    &text[start..i],
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: palette.mauve,
                        ..Default::default()
                    },
                );
                continue;
            }

            // 5. Dos puntos y comas
            if matches!(b, b':' | b',') {
                let start = i;
                i += 1;
                job.append(
                    &text[start..i],
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: palette.sapphire,
                        ..Default::default()
                    },
                );
                continue;
            }

            // 6. Números JSON
            if b.is_ascii_digit() || b == b'-' {
                let start = i;
                i += 1;
                while i < len
                    && (bytes[i].is_ascii_digit()
                        || matches!(bytes[i], b'.' | b'e' | b'E' | b'+' | b'-'))
                {
                    i += 1;
                }
                job.append(
                    &text[start..i],
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color: palette.peach,
                        ..Default::default()
                    },
                );
                continue;
            }

            // 7. Palabras clave (true, false, null) o identificadores no válidos
            if b.is_ascii_alphabetic() || b == b'_' {
                let start = i;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &text[start..i];
                let color = if matches!(word, "true" | "false" | "null") {
                    palette.mauve
                } else {
                    palette.red
                };
                job.append(
                    word,
                    0.0,
                    egui::TextFormat {
                        font_id: font_id.clone(),
                        color,
                        ..Default::default()
                    },
                );
                continue;
            }

            // 8. Cualquier otro carácter UTF-8
            let start = i;
            let ch = text[i..].chars().next().unwrap_or(' ');
            i += ch.len_utf8();
            job.append(
                &text[start..i],
                0.0,
                egui::TextFormat {
                    font_id: font_id.clone(),
                    color: palette.text,
                    ..Default::default()
                },
            );
        }

        job
    }

    /// Formatea JSON de manera inteligente (usa serde_json si es estricto, o indentación estructural si contiene {{variables}})
    fn format_json_smart(input: &str) -> String {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Ok(pretty) = serde_json::to_string_pretty(&val) {
                return pretty;
            }
        }

        // Formateador estructural tolerante a {{variables}} sin comillas
        let mut out = String::with_capacity(trimmed.len() + 128);
        let mut indent: usize = 0;
        let mut in_string = false;
        let mut escaped = false;

        let chars: Vec<char> = trimmed.chars().collect();
        let mut idx = 0;
        while idx < chars.len() {
            let ch = chars[idx];
            if in_string {
                out.push(ch);
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
                idx += 1;
                continue;
            }

            // Preservar {{variables}} intactas
            if ch == '{' && idx + 1 < chars.len() && chars[idx + 1] == '{' {
                while idx < chars.len() {
                    out.push(chars[idx]);
                    if idx > 0 && chars[idx - 1] == '}' && chars[idx] == '}' {
                        idx += 1;
                        break;
                    }
                    idx += 1;
                }
                continue;
            }

            match ch {
                '"' => {
                    in_string = true;
                    out.push(ch);
                }
                '{' | '[' => {
                    out.push(ch);
                    indent += 1;
                    out.push('\n');
                    for _ in 0..indent {
                        out.push_str("  ");
                    }
                }
                '}' | ']' => {
                    indent = indent.saturating_sub(1);
                    out.push('\n');
                    for _ in 0..indent {
                        out.push_str("  ");
                    }
                    out.push(ch);
                }
                ',' => {
                    out.push(',');
                    out.push('\n');
                    for _ in 0..indent {
                        out.push_str("  ");
                    }
                }
                ':' => {
                    out.push_str(": ");
                }
                c if c.is_whitespace() => {}
                other => {
                    out.push(other);
                }
            }
            idx += 1;
        }
        out
    }

    fn render_html_preview(ui: &mut egui::Ui, html: &str, palette: &ThemePalette) {
        let page_title = {
            let lower = html.to_lowercase();
            if let (Some(s), Some(e)) = (lower.find("<title>"), lower.find("</title>")) {
                if s + 7 <= e {
                    Some(html[s + 7..e].trim().to_string())
                } else {
                    None
                }
            } else {
                None
            }
        };

        let mut cleaned = html.to_string();
        for tag in ["script", "style", "head", "svg", "noscript"] {
            let open_pat = format!("<{}", tag);
            let close_pat = format!("</{}>", tag);
            while let Some(start) = cleaned.to_lowercase().find(&open_pat) {
                if let Some(end_rel) = cleaned[start..].to_lowercase().find(&close_pat) {
                    let end = start + end_rel + close_pat.len();
                    cleaned.replace_range(start..end, "");
                } else {
                    break;
                }
            }
        }

        egui::Frame::none()
            .fill(palette.crust)
            .inner_margin(egui::Margin::same(16.0))
            .rounding(egui::Rounding::same(8.0))
            .stroke(egui::Stroke::new(1.0_f32, palette.surface1))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("📄 Vista Renderizada HTML")
                            .strong()
                            .color(palette.mauve),
                    );
                    if let Some(title) = page_title {
                        ui.separator();
                        ui.label(
                            egui::RichText::new(format!("Título: {}", title))
                                .strong()
                                .color(palette.sapphire),
                        );
                    }
                });
                ui.separator();
                ui.add_space(6.0);

                let normalized = cleaned
                    .replace("<br>", "\n")
                    .replace("<br/>", "\n")
                    .replace("<br />", "\n")
                    .replace("</p>", "\n\n")
                    .replace("</div>", "\n")
                    .replace("</section>", "\n\n")
                    .replace("</article>", "\n\n")
                    .replace("</header>", "\n")
                    .replace("</footer>", "\n")
                    .replace("</li>", "\n")
                    .replace("</tr>", "\n")
                    .replace("</td>", "   |   ")
                    .replace("</th>", "   |   ")
                    .replace("</h1>", "\n\n")
                    .replace("</h2>", "\n\n")
                    .replace("</h3>", "\n\n")
                    .replace("</h4>", "\n");

                let mut in_tag = false;
                let mut current_tag = String::new();
                let mut text_buf = String::new();
                let mut active_heading = 0u8;
                let mut is_list_item = false;
                let mut is_link = false;
                let mut is_code_or_pre = false;
                let mut is_bold = false;

                for ch in normalized.chars() {
                    if ch == '<' {
                        let trimmed = text_buf.trim();
                        if !trimmed.is_empty() {
                            let decoded = trimmed
                                .replace("&nbsp;", " ")
                                .replace("&amp;", "&")
                                .replace("&lt;", "<")
                                .replace("&gt;", ">")
                                .replace("&quot;", "\"")
                                .replace("&#39;", "'")
                                .replace("&copy;", "©");
                            match active_heading {
                                1 => {
                                    ui.add_space(4.0);
                                    ui.heading(
                                        egui::RichText::new(decoded)
                                            .size(22.0)
                                            .strong()
                                            .color(palette.blue),
                                    );
                                    ui.add_space(4.0);
                                }
                                2 => {
                                    ui.add_space(3.0);
                                    ui.label(
                                        egui::RichText::new(decoded)
                                            .size(18.0)
                                            .strong()
                                            .color(palette.sapphire),
                                    );
                                }
                                3 | 4 => {
                                    ui.label(
                                        egui::RichText::new(decoded)
                                            .size(15.5)
                                            .strong()
                                            .color(palette.mauve),
                                    );
                                }
                                _ => {
                                    if is_code_or_pre {
                                        ui.code(decoded);
                                    } else if is_link {
                                        ui.label(
                                            egui::RichText::new(format!("🔗 {}", decoded))
                                                .underline()
                                                .color(palette.sapphire),
                                        );
                                    } else if is_list_item {
                                        ui.label(
                                            egui::RichText::new(format!("  • {}", decoded))
                                                .color(palette.text),
                                        );
                                    } else if is_bold {
                                        ui.label(
                                            egui::RichText::new(decoded)
                                                .strong()
                                                .color(palette.text),
                                        );
                                    } else {
                                        ui.label(egui::RichText::new(decoded).color(palette.text));
                                    }
                                }
                            }
                        }
                        text_buf.clear();
                        in_tag = true;
                        current_tag.clear();
                    } else if ch == '>' {
                        in_tag = false;
                        let t = current_tag
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .to_lowercase();
                        match t.as_str() {
                            "h1" => active_heading = 1,
                            "h2" => active_heading = 2,
                            "h3" => active_heading = 3,
                            "h4" => active_heading = 4,
                            "/h1" | "/h2" | "/h3" | "/h4" => active_heading = 0,
                            "li" => is_list_item = true,
                            "/li" => is_list_item = false,
                            "a" => is_link = true,
                            "/a" => is_link = false,
                            "pre" | "code" => is_code_or_pre = true,
                            "/pre" | "/code" => is_code_or_pre = false,
                            "b" | "strong" => is_bold = true,
                            "/b" | "/strong" => is_bold = false,
                            "hr" | "hr/" => {
                                ui.separator();
                            }
                            _ => {}
                        }
                    } else if in_tag {
                        current_tag.push(ch);
                    } else {
                        text_buf.push(ch);
                    }
                }

                let remaining = text_buf.trim();
                if !remaining.is_empty() {
                    ui.label(egui::RichText::new(remaining).color(palette.text));
                }
            });
    }

    fn apply_theme(ctx: &egui::Context, mode: ThemeMode) {
        let p = ThemePalette::for_mode(mode);
        let mut style = (*ctx.style()).clone();

        style.spacing.item_spacing = egui::vec2(8.0, 7.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.window_margin = egui::Margin::same(10.0);

        let mut visuals = if mode.is_light() {
            egui::Visuals::light()
        } else {
            egui::Visuals::dark()
        };

        visuals.panel_fill = p.base;
        visuals.window_fill = p.mantle;
        visuals.extreme_bg_color = p.crust;
        visuals.faint_bg_color = p.surface0;
        visuals.code_bg_color = p.crust;

        visuals.override_text_color = None;
        visuals.hyperlink_color = p.sapphire;
        visuals.warn_fg_color = p.peach;
        visuals.error_fg_color = p.red;

        visuals.selection.bg_fill = if mode.is_light() {
            egui::Color32::from_rgb(180, 205, 255)
        } else {
            p.active_bg
        };
        visuals.selection.stroke = egui::Stroke::new(1.5_f32, p.text);

        let rounding = egui::Rounding::same(6.0);
        visuals.window_rounding = egui::Rounding::same(8.0);
        visuals.menu_rounding = rounding;

        visuals.widgets.noninteractive.bg_fill = p.base;
        visuals.widgets.noninteractive.weak_bg_fill = p.mantle;
        visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, p.surface1);
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.2_f32, p.text);
        visuals.widgets.noninteractive.rounding = rounding;

        visuals.widgets.inactive.bg_fill = p.surface0;
        visuals.widgets.inactive.weak_bg_fill = p.surface0;
        visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, p.surface1);
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.2_f32, p.text);
        visuals.widgets.inactive.rounding = rounding;

        visuals.widgets.hovered.bg_fill = p.surface1;
        visuals.widgets.hovered.weak_bg_fill = p.surface1;
        visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.5_f32, p.mauve);
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5_f32, p.text);
        visuals.widgets.hovered.rounding = rounding;

        visuals.widgets.active.bg_fill = if mode.is_light() {
            egui::Color32::from_rgb(160, 190, 250)
        } else {
            p.active_bg
        };
        visuals.widgets.active.weak_bg_fill = visuals.widgets.active.bg_fill;
        visuals.widgets.active.bg_stroke = egui::Stroke::new(1.5_f32, p.mauve);
        visuals.widgets.active.fg_stroke = egui::Stroke::new(2.0_f32, p.text);
        visuals.widgets.active.rounding = rounding;

        visuals.widgets.open.bg_fill = p.surface1;
        visuals.widgets.open.weak_bg_fill = p.surface1;
        visuals.widgets.open.bg_stroke = egui::Stroke::new(1.5_f32, p.mauve);
        visuals.widgets.open.fg_stroke = egui::Stroke::new(1.5_f32, p.text);
        visuals.widgets.open.rounding = rounding;

        style.visuals = visuals;
        ctx.set_style(style);
    }

    fn storage_file_path() -> PathBuf {
        let mut dir = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        dir.push("rustman");
        let _ = fs::create_dir_all(&dir);
        dir.push("rustman_data.json");
        dir
    }

    fn load_storage_from_disk() -> Option<AppStorage> {
        let path = Self::storage_file_path();
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn save_storage_to_disk(&self) {
        let path = Self::storage_file_path();
        if let Ok(json) = serde_json::to_string_pretty(&self.storage) {
            let _ = fs::write(path, json);
        }
    }

    fn active_environment(&self) -> Option<Environment> {
        self.storage
            .active_env_index
            .and_then(|idx| self.storage.environments.get(idx).cloned())
    }

    fn trigger_request(&mut self, ctx: &egui::Context) {
        let tab = &mut self.tabs[self.active_tab_idx];
        if tab.is_loading {
            return;
        }
        tab.is_loading = true;
        let tab_id = tab.tab_id;
        let req_clone = tab.request.clone();

        self.status_message = Some("⏳ Enviando petición HTTP...".to_string());

        let env_clone = self.active_environment();
        let cookies_clone = self.storage.cookies.clone();
        let tx_clone = self.tx.clone();
        let ctx_clone = ctx.clone();

        self.rt.spawn(async move {
            let resp = send_http_request(req_clone, env_clone, cookies_clone).await;
            let _ = tx_clone.send((tab_id, resp));
            ctx_clone.request_repaint();
        });
    }

    fn render_key_value_table(
        ui: &mut egui::Ui,
        id_salt: &str,
        items: &mut Vec<KeyValue>,
        text_color: egui::Color32,
        enable_header_autocomplete: bool,
    ) {
        egui::ScrollArea::vertical()
            .id_salt(id_salt)
            .max_height(210.0)
            .show(ui, |ui| {
                let mut to_remove = None;

                egui::Grid::new(format!("{}_grid", id_salt))
                    .striped(true)
                    .num_columns(5)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        Self::col_header(ui, "Activo", text_color);
                        Self::col_header(ui, "Clave (Key)", text_color);
                        Self::col_header(ui, "Valor (Value)", text_color);
                        Self::col_header(ui, "Descripción", text_color);
                        Self::col_header(ui, "", text_color);
                        ui.end_row();

                        for (idx, item) in items.iter_mut().enumerate() {
                            ui.checkbox(&mut item.enabled, "");

                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut item.key)
                                        .hint_text(if enable_header_autocomplete {
                                            "Ej. Accept, Content-Type..."
                                        } else {
                                            "Clave"
                                        })
                                        .desired_width(150.0),
                                );

                                if enable_header_autocomplete {
                                    egui::ComboBox::from_id_salt(format!(
                                        "{}_key_preset_{}",
                                        id_salt, idx
                                    ))
                                    .width(22.0)
                                    .selected_text("⚡")
                                    .show_ui(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new("Headers estándar")
                                                .strong()
                                                .color(text_color),
                                        );
                                        ui.separator();
                                        let query = item.key.trim().to_lowercase();
                                        for (h_name, default_vals) in COMMON_HEADERS {
                                            if query.is_empty()
                                                || h_name.to_lowercase().contains(&query)
                                            {
                                                if ui.selectable_label(false, *h_name).clicked() {
                                                    item.key = (*h_name).to_string();
                                                    if item.value.trim().is_empty() {
                                                        if let Some(first_val) =
                                                            default_vals.first()
                                                        {
                                                            item.value = (*first_val).to_string();
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    });
                                }
                            });

                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut item.value)
                                        .hint_text("Valor")
                                        .desired_width(210.0),
                                );

                                if enable_header_autocomplete {
                                    let suggestions = Self::header_values_for_key(&item.key);
                                    egui::ComboBox::from_id_salt(format!(
                                        "{}_val_preset_{}",
                                        id_salt, idx
                                    ))
                                    .width(22.0)
                                    .selected_text("▾")
                                    .show_ui(ui, |ui| {
                                        ui.label(
                                            egui::RichText::new(format!(
                                                "Valores para '{}'",
                                                if item.key.is_empty() {
                                                    "Header"
                                                } else {
                                                    &item.key
                                                }
                                            ))
                                            .strong()
                                            .color(text_color),
                                        );
                                        ui.separator();
                                        for &sug in suggestions {
                                            if ui.selectable_label(item.value == sug, sug).clicked()
                                            {
                                                item.value = sug.to_string();
                                            }
                                        }
                                    });
                                }
                            });

                            ui.add(
                                egui::TextEdit::singleline(&mut item.description)
                                    .hint_text("Opcional")
                                    .desired_width(130.0),
                            );
                            if ui.button("🗑").on_hover_text("Eliminar fila").clicked() {
                                to_remove = Some(idx);
                            }
                            ui.end_row();
                        }
                    });

                if let Some(idx) = to_remove {
                    items.remove(idx);
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("+ Agregar fila").clicked() {
                        items.push(KeyValue::empty());
                    }
                    if !items.is_empty() && ui.button("🗑 Borrar todas").clicked() {
                        items.clear();
                    }
                });
            });
    }

    fn render_multipart_table(
        ui: &mut egui::Ui,
        items: &mut Vec<MultipartField>,
        text_color: egui::Color32,
    ) {
        egui::ScrollArea::vertical()
            .id_salt("multipart_form_scroll")
            .max_height(210.0)
            .show(ui, |ui| {
                let mut to_remove = None;

                egui::Grid::new("multipart_form_grid")
                    .striped(true)
                    .num_columns(6)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        Self::col_header(ui, "Activo", text_color);
                        Self::col_header(ui, "Campo (Key)", text_color);
                        Self::col_header(ui, "Tipo", text_color);
                        Self::col_header(ui, "Valor / Archivo", text_color);
                        Self::col_header(ui, "Descripción", text_color);
                        Self::col_header(ui, "", text_color);
                        ui.end_row();

                        for (idx, item) in items.iter_mut().enumerate() {
                            ui.checkbox(&mut item.enabled, "");
                            ui.add(
                                egui::TextEdit::singleline(&mut item.key)
                                    .hint_text("Nombre del campo")
                                    .desired_width(140.0),
                            );

                            egui::ComboBox::from_id_salt(format!("mp_kind_{}", idx))
                                .width(85.0)
                                .selected_text(match item.kind {
                                    MultipartKind::Text => "📝 Texto",
                                    MultipartKind::File => "📎 Archivo",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut item.kind,
                                        MultipartKind::Text,
                                        "📝 Texto",
                                    );
                                    ui.selectable_value(
                                        &mut item.kind,
                                        MultipartKind::File,
                                        "📎 Archivo",
                                    );
                                });

                            match item.kind {
                                MultipartKind::Text => {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut item.value)
                                            .hint_text("Valor de texto")
                                            .desired_width(240.0),
                                    );
                                }
                                MultipartKind::File => {
                                    ui.horizontal(|ui| {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut item.value)
                                                .hint_text("Ruta del archivo...")
                                                .desired_width(165.0),
                                        );
                                        if ui.button("📂 Elegir...").clicked() {
                                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                                item.value = path.display().to_string();
                                            }
                                        }
                                    });
                                }
                            }

                            ui.add(
                                egui::TextEdit::singleline(&mut item.description)
                                    .hint_text("Opcional")
                                    .desired_width(120.0),
                            );

                            if ui.button("🗑").on_hover_text("Eliminar fila").clicked() {
                                to_remove = Some(idx);
                            }
                            ui.end_row();
                        }
                    });

                if let Some(idx) = to_remove {
                    items.remove(idx);
                }

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("+ Agregar campo Multipart").clicked() {
                        items.push(MultipartField::empty());
                    }
                    if !items.is_empty() && ui.button("🗑 Borrar todos").clicked() {
                        items.clear();
                    }
                });
            });
    }
}

impl eframe::App for RustmanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let theme = self.storage.theme;
        let palette = ThemePalette::for_mode(theme);

        if self.tabs.is_empty() {
            self.tabs.push(WorkspaceTab::new(HttpRequestItem::default()));
            self.active_tab_idx = 0;
        }
        if self.active_tab_idx >= self.tabs.len() {
            self.active_tab_idx = self.tabs.len() - 1;
        }

        // Verificar si llegó respuesta del hilo asíncrono para cualquiera de las pestañas
        if let Ok((tab_id, resp)) = self.rx.try_recv() {
            self.status_message = None;

            // Guardar nuevas cookies Set-Cookie en el Cookie Jar global
            for (ck, cv) in &resp.cookies {
                if let Some(existing) = self
                    .storage
                    .cookies
                    .iter_mut()
                    .find(|c| c.key.eq_ignore_ascii_case(ck))
                {
                    existing.value = cv.clone();
                } else {
                    self.storage.cookies.push(KeyValue::new(ck, cv));
                }
            }

            // Guardar variables extraídas post-request en el entorno activo
            if !resp.extracted_vars.is_empty() {
                if self.storage.environments.is_empty() {
                    self.storage.environments.push(Environment::default());
                    self.storage.active_env_index = Some(0);
                }
                let env_idx = self.storage.active_env_index.unwrap_or(0);
                if let Some(env) = self.storage.environments.get_mut(env_idx) {
                    for (var_name, var_val) in &resp.extracted_vars {
                        if let Some(existing_var) =
                            env.variables.iter_mut().find(|v| v.key.trim() == var_name)
                        {
                            existing_var.value = var_val.clone();
                            existing_var.enabled = true;
                        } else {
                            env.variables.push(KeyValue::new(var_name, var_val));
                        }
                    }
                }
                self.status_message = Some(format!(
                    "✨ {} variable(s) extraídas y guardadas en el entorno activo.",
                    resp.extracted_vars.len()
                ));
            }

            if let Some(target_tab) = self.tabs.iter_mut().find(|t| t.tab_id == tab_id) {
                target_tab.is_loading = false;
                let entry = HistoryEntry {
                    timestamp: Local::now(),
                    request: target_tab.request.clone(),
                    status_code: if resp.status_code > 0 {
                        Some(resp.status_code)
                    } else {
                        None
                    },
                    elapsed_ms: Some(resp.elapsed_ms),
                };
                self.storage.history.insert(0, entry);
                if self.storage.history.len() > 50 {
                    self.storage.history.truncate(50);
                }
                target_tab.response = Some(resp);
            }
            self.save_storage_to_disk();
        }

        // Atajos de teclado globales
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Enter)) {
            self.trigger_request(ctx);
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::S)) {
            self.save_current_request_to_collection();
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::T)) {
            self.tabs.push(WorkspaceTab::new(HttpRequestItem::default()));
            self.active_tab_idx = self.tabs.len() - 1;
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::W)) {
            if self.tabs.len() > 1 {
                self.tabs.remove(self.active_tab_idx);
                if self.active_tab_idx >= self.tabs.len() {
                    self.active_tab_idx = self.tabs.len() - 1;
                }
            }
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::L)) {
            self.focus_url_next_frame = true;
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            self.focus_search_next_frame = true;
        }

        // Barra superior (Header global + Selector de Tema Catppuccin y Entorno)
        egui::TopBottomPanel::top("top_bar")
            .frame(
                egui::Frame::none()
                    .fill(palette.mantle)
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0))
                    .stroke(egui::Stroke::new(1.0_f32, palette.surface0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading(
                        egui::RichText::new("🐾 RUSTMAN")
                            .strong()
                            .size(20.0)
                            .color(palette.mauve),
                    );
                    ui.separator();
                    ui.label(
                        egui::RichText::new(theme.label())
                            .color(palette.subtext)
                            .italics(),
                    );
                    ui.separator();
                    ui.small(
                        egui::RichText::new(
                            "⌨ Ctrl+Enter: Enviar | Ctrl+S: Guardar | Ctrl+T: Pestaña | Ctrl+L: URL | Ctrl+F: Buscar",
                        )
                        .color(palette.overlay),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut selected_theme = self.storage.theme;
                        egui::ComboBox::from_id_salt("catppuccin_theme_selector")
                            .selected_text(format!("🎨 {}", selected_theme.label()))
                            .show_ui(ui, |ui| {
                                for mode in ThemeMode::all() {
                                    if ui
                                        .selectable_value(&mut selected_theme, *mode, mode.label())
                                        .clicked()
                                    {
                                        self.storage.theme = selected_theme;
                                        Self::apply_theme(ctx, self.storage.theme);
                                        self.save_storage_to_disk();
                                    }
                                }
                            });

                        let quick_toggle_label = if theme.is_light() {
                            "🌙 Modo Oscuro"
                        } else {
                            "☀ Modo Claro"
                        };
                        if ui
                            .button(quick_toggle_label)
                            .on_hover_text("Alternar rápido entre Catppuccin Mocha y Latte")
                            .clicked()
                        {
                            self.storage.theme = if theme.is_light() {
                                ThemeMode::CatppuccinMocha
                            } else {
                                ThemeMode::CatppuccinLatte
                            };
                            Self::apply_theme(ctx, self.storage.theme);
                            self.save_storage_to_disk();
                        }

                        ui.separator();

                        let current_env_name = self
                            .storage
                            .active_env_index
                            .and_then(|i| self.storage.environments.get(i))
                            .map(|e| e.name.as_str())
                            .unwrap_or("Sin Entorno");

                        egui::ComboBox::from_id_salt("env_selector")
                            .selected_text(format!("🌍 Entorno: {}", current_env_name))
                            .show_ui(ui, |ui| {
                                if ui
                                    .selectable_label(
                                        self.storage.active_env_index.is_none(),
                                        "Sin Entorno",
                                    )
                                    .clicked()
                                {
                                    self.storage.active_env_index = None;
                                    self.save_storage_to_disk();
                                }
                                for (idx, env) in self.storage.environments.iter().enumerate() {
                                    if ui
                                        .selectable_label(
                                            self.storage.active_env_index == Some(idx),
                                            &env.name,
                                        )
                                        .clicked()
                                    {
                                        self.storage.active_env_index = Some(idx);
                                        self.save_storage_to_disk();
                                    }
                                }
                            });
                    });
                });
            });

        // Panel lateral izquierdo (Colecciones, Historial, Entornos, Cookies)
        egui::SidePanel::left("left_sidebar")
            .resizable(true)
            .default_width(300.0)
            .frame(
                egui::Frame::none()
                    .fill(palette.mantle)
                    .inner_margin(egui::Margin::same(12.0))
                    .stroke(egui::Stroke::new(1.0_f32, palette.surface0)),
            )
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut self.sidebar_tab,
                        SidebarTab::Collections,
                        "📁 Colecciones",
                    );
                    ui.selectable_value(
                        &mut self.sidebar_tab,
                        SidebarTab::History,
                        "🕒 Historial",
                    );
                    ui.selectable_value(
                        &mut self.sidebar_tab,
                        SidebarTab::Environments,
                        "⚙ Entornos",
                    );
                    ui.selectable_value(
                        &mut self.sidebar_tab,
                        SidebarTab::Cookies,
                        format!("🍪 Cookies ({})", self.storage.cookies.len()),
                    );
                });
                ui.separator();

                match self.sidebar_tab {
                    SidebarTab::Collections => {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_collection_name)
                                    .hint_text("Nueva colección...")
                                    .desired_width(160.0),
                            );
                            if ui.button("➕ Crear").clicked()
                                && !self.new_collection_name.trim().is_empty()
                            {
                                self.storage.collections.push(Collection {
                                    id: Uuid::new_v4(),
                                    name: self.new_collection_name.trim().to_string(),
                                    requests: Vec::new(),
                                });
                                self.new_collection_name.clear();
                                self.save_storage_to_disk();
                            }
                        });

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button("📥 Importar JSON / Postman")
                                .on_hover_text(
                                    "Importar colección en formato Rustman o Postman v2.1",
                                )
                                .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .add_filter("JSON", &["json"])
                                    .pick_file()
                                {
                                    match fs::read_to_string(&path) {
                                        Ok(content) => match import_collections_from_json(&content)
                                        {
                                            Ok(cols) => {
                                                let count = cols.len();
                                                self.storage.collections.extend(cols);
                                                self.save_storage_to_disk();
                                                self.status_message = Some(format!(
                                                    "✅ {} colección(es) importada(s) correctamente.",
                                                    count
                                                ));
                                            }
                                            Err(err) => {
                                                self.status_message = Some(format!("❌ {}", err));
                                            }
                                        },
                                        Err(e) => {
                                            self.status_message = Some(format!(
                                                "❌ Error leyendo archivo: {}",
                                                e
                                            ));
                                        }
                                    }
                                }
                            }

                            if ui
                                .button("📤 Exportar")
                                .on_hover_text("Exportar todas las colecciones a un archivo .json")
                                .clicked()
                            {
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_file_name("rustman_collections.json")
                                    .add_filter("JSON", &["json"])
                                    .save_file()
                                {
                                    if let Ok(json) =
                                        serde_json::to_string_pretty(&self.storage.collections)
                                    {
                                        let _ = fs::write(&path, json);
                                        self.status_message = Some(format!(
                                            "✅ Colecciones exportadas a {}",
                                            path.display()
                                        ));
                                    }
                                }
                            }
                        });

                        ui.add_space(4.0);
                        if ui.button("➕ Nueva Pestaña en Blanco (Ctrl+T)").clicked() {
                            self.tabs.push(WorkspaceTab::new(HttpRequestItem::default()));
                            self.active_tab_idx = self.tabs.len() - 1;
                        }
                        ui.separator();

                        let mut req_to_load: Option<HttpRequestItem> = None;
                        let mut delete_req: Option<(usize, usize)> = None;
                        let mut delete_col: Option<usize> = None;

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (c_idx, col) in self.storage.collections.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    if ui
                                        .small_button("🗑")
                                        .on_hover_text("Eliminar colección completa")
                                        .clicked()
                                    {
                                        delete_col = Some(c_idx);
                                    }
                                    egui::CollapsingHeader::new(
                                        egui::RichText::new(format!("📂 {}", col.name)).strong(),
                                    )
                                    .id_salt(format!("col_header_{}", col.id))
                                    .default_open(true)
                                    .show(ui, |ui| {
                                        for (r_idx, req) in col.requests.iter().enumerate() {
                                            ui.horizontal(|ui| {
                                                ui.colored_label(
                                                    req.method.color(theme),
                                                    egui::RichText::new(req.method.as_str())
                                                        .monospace()
                                                        .strong(),
                                                );
                                                if ui.selectable_label(false, &req.name).clicked() {
                                                    req_to_load = Some(req.clone());
                                                }
                                                if ui
                                                    .small_button("✖")
                                                    .on_hover_text("Eliminar petición")
                                                    .clicked()
                                                {
                                                    delete_req = Some((c_idx, r_idx));
                                                }
                                            });
                                        }
                                    });
                                });
                            }
                        });

                        if let Some(req) = req_to_load {
                            self.open_or_focus_request(req);
                        }
                        if let Some((c_idx, r_idx)) = delete_req {
                            if let Some(col) = self.storage.collections.get_mut(c_idx) {
                                col.requests.remove(r_idx);
                                self.save_storage_to_disk();
                            }
                        }
                        if let Some(c_idx) = delete_col {
                            self.storage.collections.remove(c_idx);
                            if self.selected_collection_idx >= self.storage.collections.len() {
                                self.selected_collection_idx = 0;
                            }
                            self.save_storage_to_disk();
                        }
                    }

                    SidebarTab::History => {
                        if ui.button("🗑 Limpiar Todo el Historial").clicked() {
                            self.storage.history.clear();
                            self.save_storage_to_disk();
                        }
                        ui.separator();

                        let mut req_to_load: Option<HttpRequestItem> = None;
                        let mut delete_hist_idx: Option<usize> = None;

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (h_idx, item) in self.storage.history.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.colored_label(
                                        item.request.method.color(theme),
                                        egui::RichText::new(item.request.method.as_str())
                                            .monospace()
                                            .strong(),
                                    );
                                    if let Some(code) = item.status_code {
                                        let color = if (200..300).contains(&code) {
                                            palette.green
                                        } else {
                                            palette.red
                                        };
                                        ui.colored_label(color, format!("[{}]", code));
                                    }
                                    let label = format!(
                                        "{} ({})",
                                        item.request.url,
                                        item.timestamp.format("%H:%M:%S")
                                    );
                                    if ui.selectable_label(false, label).clicked() {
                                        req_to_load = Some(item.request.clone());
                                    }
                                    if ui
                                        .small_button("✖")
                                        .on_hover_text("Borrar del historial")
                                        .clicked()
                                    {
                                        delete_hist_idx = Some(h_idx);
                                    }
                                });
                            }
                        });

                        if let Some(req) = req_to_load {
                            self.open_or_focus_request(req);
                        }
                        if let Some(h_idx) = delete_hist_idx {
                            self.storage.history.remove(h_idx);
                            self.save_storage_to_disk();
                        }
                    }

                    SidebarTab::Environments => {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_env_name)
                                    .hint_text("Nuevo entorno...")
                                    .desired_width(150.0),
                            );
                            if ui.button("➕").clicked() && !self.new_env_name.trim().is_empty() {
                                self.storage.environments.push(Environment {
                                    id: Uuid::new_v4(),
                                    name: self.new_env_name.trim().to_string(),
                                    variables: vec![KeyValue::empty()],
                                });
                                self.new_env_name.clear();
                                self.save_storage_to_disk();
                            }
                        });
                        ui.separator();

                        let mut delete_env_idx: Option<usize> = None;

                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (e_idx, env) in self.storage.environments.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    if ui
                                        .small_button("🗑")
                                        .on_hover_text("Eliminar entorno completo")
                                        .clicked()
                                    {
                                        delete_env_idx = Some(e_idx);
                                    }
                                    egui::CollapsingHeader::new(format!("🌍 {}", env.name))
                                        .id_salt(format!("env_header_{}", env.id))
                                        .default_open(true)
                                        .show(ui, |ui| {
                                            let mut remove_idx = None;
                                            for (i, var) in env.variables.iter_mut().enumerate() {
                                                ui.horizontal(|ui| {
                                                    ui.checkbox(&mut var.enabled, "");
                                                    ui.add(
                                                        egui::TextEdit::singleline(&mut var.key)
                                                            .hint_text("variable")
                                                            .desired_width(80.0),
                                                    );
                                                    ui.add(
                                                        egui::TextEdit::singleline(&mut var.value)
                                                            .hint_text("valor")
                                                            .desired_width(110.0),
                                                    );
                                                    if ui
                                                        .small_button("✖")
                                                        .on_hover_text("Eliminar variable")
                                                        .clicked()
                                                    {
                                                        remove_idx = Some(i);
                                                    }
                                                });
                                            }
                                            if let Some(i) = remove_idx {
                                                env.variables.remove(i);
                                            }
                                            if ui.small_button("+ Variable").clicked() {
                                                env.variables.push(KeyValue::empty());
                                            }
                                        });
                                });
                            }
                        });

                        if let Some(e_idx) = delete_env_idx {
                            self.storage.environments.remove(e_idx);
                            if let Some(active) = self.storage.active_env_index {
                                if active == e_idx {
                                    self.storage.active_env_index = None;
                                } else if active > e_idx {
                                    self.storage.active_env_index = Some(active - 1);
                                }
                            }
                            self.save_storage_to_disk();
                        }

                        if ui.button("💾 Guardar Entornos").clicked() {
                            self.save_storage_to_disk();
                        }
                    }

                    SidebarTab::Cookies => {
                        ui.label(
                            egui::RichText::new(
                                "Cookies enviadas automáticamente en tus peticiones:",
                            )
                            .color(palette.subtext),
                        );
                        ui.add_space(4.0);

                        let mut remove_cookie_idx = None;
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for (idx, c) in self.storage.cookies.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut c.enabled, "");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut c.key)
                                            .hint_text("nombre")
                                            .desired_width(85.0),
                                    );
                                    ui.add(
                                        egui::TextEdit::singleline(&mut c.value)
                                            .hint_text("valor")
                                            .desired_width(110.0),
                                    );
                                    if ui.small_button("✖").clicked() {
                                        remove_cookie_idx = Some(idx);
                                    }
                                });
                            }
                        });
                        if let Some(idx) = remove_cookie_idx {
                            self.storage.cookies.remove(idx);
                            self.save_storage_to_disk();
                        }

                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if ui.button("+ Cookie").clicked() {
                                self.storage.cookies.push(KeyValue::empty());
                                self.save_storage_to_disk();
                            }
                            if !self.storage.cookies.is_empty()
                                && ui.button("🗑 Limpiar Jar").clicked()
                            {
                                self.storage.cookies.clear();
                                self.save_storage_to_disk();
                            }
                        });
                    }
                }
            });

        // Panel Central: Barra de Pestañas Múltiples + Constructor de Petición + Visor de Respuesta
        egui::CentralPanel::default().show(ctx, |ui| {
            // 1. Barra de Pestañas Múltiples (Multi-Tab Workspace)
            let mut tab_to_close: Option<usize> = None;
            let mut open_new_tab = false;

            egui::ScrollArea::horizontal()
                .id_salt("workspace_tabs_scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (idx, tab) in self.tabs.iter().enumerate() {
                            let is_active = idx == self.active_tab_idx;
                            let method_str = tab.request.method.as_str();
                            let dirty_mark = if tab.dirty { " •" } else { "" };
                            let loading_mark = if tab.is_loading { " ⏳" } else { "" };
                            let title = format!(
                                "{} {}{}{}",
                                method_str, tab.request.name, dirty_mark, loading_mark
                            );

                            egui::Frame::none()
                                .fill(if is_active {
                                    palette.surface0
                                } else {
                                    palette.mantle
                                })
                                .rounding(egui::Rounding::same(6.0))
                                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
                                .stroke(egui::Stroke::new(
                                    if is_active { 1.5_f32 } else { 1.0_f32 },
                                    if is_active {
                                        palette.mauve
                                    } else {
                                        palette.surface1
                                    },
                                ))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .selectable_label(
                                                is_active,
                                                egui::RichText::new(title)
                                                    .color(if is_active {
                                                        palette.text
                                                    } else {
                                                        palette.subtext
                                                    })
                                                    .strong(),
                                            )
                                            .clicked()
                                        {
                                            self.active_tab_idx = idx;
                                        }
                                        if self.tabs.len() > 1
                                            && ui
                                                .small_button("✖")
                                                .on_hover_text("Cerrar pestaña (Ctrl+W)")
                                                .clicked()
                                        {
                                            tab_to_close = Some(idx);
                                        }
                                    });
                                });
                        }

                        if ui
                            .button("➕")
                            .on_hover_text("Nueva pestaña de petición (Ctrl+T)")
                            .clicked()
                        {
                            open_new_tab = true;
                        }
                    });
                });

            if let Some(close_idx) = tab_to_close {
                self.tabs.remove(close_idx);
                if self.active_tab_idx >= self.tabs.len() {
                    self.active_tab_idx = self.tabs.len() - 1;
                }
            }
            if open_new_tab {
                self.tabs.push(WorkspaceTab::new(HttpRequestItem::default()));
                self.active_tab_idx = self.tabs.len() - 1;
            }

            ui.separator();

            let active_env = self.active_environment();
            let mut should_trigger_request = false;
            let mut should_save_to_collection = false;

            // Fila 1: Nombre de la petición, Guardado en Colección y Botón rápido de cURL
            ui.horizontal(|ui| {
                ui.label("Nombre:");
                let name_edit = ui.add(
                    egui::TextEdit::singleline(
                        &mut self.tabs[self.active_tab_idx].request.name,
                    )
                    .desired_width(210.0),
                );
                if name_edit.changed() {
                    self.tabs[self.active_tab_idx].dirty = true;
                }

                if !self.storage.collections.is_empty() {
                    if self.selected_collection_idx >= self.storage.collections.len() {
                        self.selected_collection_idx = 0;
                    }
                    egui::ComboBox::from_id_salt("save_col_combo")
                        .selected_text(&self.storage.collections[self.selected_collection_idx].name)
                        .show_ui(ui, |ui| {
                            for (idx, col) in self.storage.collections.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.selected_collection_idx,
                                    idx,
                                    &col.name,
                                );
                            }
                        });

                    if ui
                        .button("💾 Guardar (Ctrl+S)")
                        .on_hover_text("Guardar o actualizar en la colección")
                        .clicked()
                    {
                        should_save_to_collection = true;
                    }
                }

                if ui
                    .button("📥 Importar cURL")
                    .on_hover_text("Pegar un comando cURL para rellenar esta petición")
                    .clicked()
                {
                    self.req_tab = RequestTab::CodeGen;
                }
            });

            if should_save_to_collection {
                self.save_current_request_to_collection();
            }

            ui.add_space(4.0);

            // Fila 2: Selector de Método + Barra de URL + Botón ENVIAR
            ui.horizontal(|ui| {
                let tab = &mut self.tabs[self.active_tab_idx];
                egui::ComboBox::from_id_salt("http_method_combo")
                    .width(95.0)
                    .selected_text(
                        egui::RichText::new(tab.request.method.as_str())
                            .color(tab.request.method.color(theme))
                            .strong(),
                    )
                    .show_ui(ui, |ui| {
                        for m in HttpMethod::all() {
                            if ui
                                .selectable_value(
                                    &mut tab.request.method,
                                    *m,
                                    egui::RichText::new(m.as_str())
                                        .color(m.color(theme))
                                        .strong(),
                                )
                                .clicked()
                            {
                                tab.dirty = true;
                            }
                        }
                    });

                let url_input = ui.add(
                    egui::TextEdit::singleline(&mut tab.request.url)
                        .hint_text("https://api.ejemplo.com/v1/recurso (soporta {{variables}})")
                        .desired_width(ui.available_width() - 110.0),
                );

                if self.focus_url_next_frame {
                    url_input.request_focus();
                    self.focus_url_next_frame = false;
                }

                if url_input.changed() {
                    tab.dirty = true;
                    Self::sync_url_to_params(&tab.request.url, &mut tab.request.query_params);
                }

                if url_input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    should_trigger_request = true;
                }

                let send_btn = egui::Button::new(
                    egui::RichText::new(if tab.is_loading {
                        "⏳ Enviando..."
                    } else {
                        "🚀 Enviar"
                    })
                    .strong()
                    .color(palette.on_accent),
                )
                .fill(palette.blue);

                if ui.add_enabled(!tab.is_loading, send_btn).clicked() {
                    should_trigger_request = true;
                }
            });

            if should_trigger_request {
                self.trigger_request(ctx);
            }

            if let Some(msg) = &self.status_message {
                ui.small(
                    egui::RichText::new(msg)
                        .italics()
                        .color(palette.sapphire),
                );
            }

            ui.separator();

            // Pestañas de configuración de la Petición
            {
                let req = &self.tabs[self.active_tab_idx].request;
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(
                        &mut self.req_tab,
                        RequestTab::Params,
                        format!("Query Params ({})", req.query_params.len()),
                    );
                    ui.selectable_value(&mut self.req_tab, RequestTab::Auth, "🔑 Autorización");
                    ui.selectable_value(
                        &mut self.req_tab,
                        RequestTab::Headers,
                        format!("Headers ({})", req.headers.len()),
                    );
                    ui.selectable_value(&mut self.req_tab, RequestTab::Body, "📦 Body");
                    ui.selectable_value(
                        &mut self.req_tab,
                        RequestTab::TestsAndVars,
                        format!(
                            "🧪 Tests y Variables ({})",
                            req.assertions.len() + req.extractions.len()
                        ),
                    );
                    ui.selectable_value(
                        &mut self.req_tab,
                        RequestTab::CodeGen,
                        "💻 Código / cURL",
                    );
                    ui.selectable_value(&mut self.req_tab, RequestTab::Settings, "⚙ Opciones");
                    ui.selectable_value(
                        &mut self.req_tab,
                        RequestTab::EnvPreview,
                        "🔍 Vista Previa URL",
                    );
                });
            }
            ui.separator();

            // Contenido de la pestaña activa de la Petición
            egui::Frame::none().show(ui, |ui| match self.req_tab {
                RequestTab::Params => {
                    let tab = &mut self.tabs[self.active_tab_idx];
                    let prev_snapshot: Vec<(bool, String, String)> = tab
                        .request
                        .query_params
                        .iter()
                        .map(|kv| (kv.enabled, kv.key.clone(), kv.value.clone()))
                        .collect();

                    Self::render_key_value_table(
                        ui,
                        "query_params_table",
                        &mut tab.request.query_params,
                        palette.text,
                        false,
                    );

                    let curr_snapshot: Vec<(bool, String, String)> = tab
                        .request
                        .query_params
                        .iter()
                        .map(|kv| (kv.enabled, kv.key.clone(), kv.value.clone()))
                        .collect();

                    if prev_snapshot != curr_snapshot {
                        tab.dirty = true;
                        Self::sync_params_to_url(
                            &mut tab.request.url,
                            &tab.request.query_params,
                        );
                    }
                }
                RequestTab::Headers => {
                    let tab = &mut self.tabs[self.active_tab_idx];
                    Self::render_key_value_table(
                        ui,
                        "headers_table",
                        &mut tab.request.headers,
                        palette.text,
                        true,
                    );
                }
                RequestTab::Auth => {
                    let tab = &mut self.tabs[self.active_tab_idx];
                    ui.horizontal(|ui| {
                        ui.label("Tipo de Autenticación:");
                        if ui
                            .selectable_label(
                                matches!(tab.request.auth, AuthConfig::None),
                                "Ninguna",
                            )
                            .clicked()
                        {
                            tab.request.auth = AuthConfig::None;
                        }
                        if ui
                            .selectable_label(
                                matches!(tab.request.auth, AuthConfig::Bearer { .. }),
                                "Bearer Token",
                            )
                            .clicked()
                        {
                            tab.request.auth = AuthConfig::Bearer {
                                token: String::new(),
                            };
                        }
                        if ui
                            .selectable_label(
                                matches!(tab.request.auth, AuthConfig::Basic { .. }),
                                "Basic Auth",
                            )
                            .clicked()
                        {
                            tab.request.auth = AuthConfig::Basic {
                                username: String::new(),
                                password: String::new(),
                            };
                        }
                        if ui
                            .selectable_label(
                                matches!(tab.request.auth, AuthConfig::ApiKey { .. }),
                                "API Key",
                            )
                            .clicked()
                        {
                            tab.request.auth = AuthConfig::ApiKey {
                                key: "X-API-Key".to_string(),
                                value: String::new(),
                                in_query: false,
                            };
                        }
                    });
                    ui.add_space(6.0);

                    match &mut tab.request.auth {
                        AuthConfig::None => {
                            ui.label("Esta petición no utiliza autenticación adicional.");
                        }
                        AuthConfig::Bearer { token } => {
                            ui.horizontal(|ui| {
                                ui.label("Token Bearer:");
                                ui.add(
                                    egui::TextEdit::singleline(token)
                                        .hint_text("Ej. eyJhbGciOi... o {{api_token}}")
                                        .desired_width(400.0),
                                );
                            });
                        }
                        AuthConfig::Basic { username, password } => {
                            ui.horizontal(|ui| {
                                ui.label("Usuario:");
                                ui.text_edit_singleline(username);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Contraseña:");
                                ui.add(egui::TextEdit::singleline(password).password(true));
                            });
                        }
                        AuthConfig::ApiKey {
                            key,
                            value,
                            in_query,
                        } => {
                            ui.horizontal(|ui| {
                                ui.label("Nombre de Clave:");
                                ui.text_edit_singleline(key);
                            });
                            ui.horizontal(|ui| {
                                ui.label("Valor:");
                                ui.text_edit_singleline(value);
                            });
                            ui.checkbox(
                                in_query,
                                "Enviar como Query Param (si se desmarca se envía en Header)",
                            );
                        }
                    }
                }
                RequestTab::Body => {
                    let tab = &mut self.tabs[self.active_tab_idx];
                    ui.horizontal_wrapped(|ui| {
                        ui.radio_value(&mut tab.request.body_type, BodyType::None, "None");
                        ui.radio_value(&mut tab.request.body_type, BodyType::Json, "JSON");
                        ui.radio_value(
                            &mut tab.request.body_type,
                            BodyType::RawText,
                            "Texto Plano",
                        );
                        ui.radio_value(
                            &mut tab.request.body_type,
                            BodyType::FormUrlEncoded,
                            "x-www-form-urlencoded",
                        );
                        ui.radio_value(
                            &mut tab.request.body_type,
                            BodyType::MultipartForm,
                            "multipart/form-data",
                        );

                        if tab.request.body_type == BodyType::Json {
                            ui.separator();
                            if ui
                                .button("✨ Formatear JSON")
                                .on_hover_text("Indentar y ordenar el formato del JSON")
                                .clicked()
                            {
                                tab.request.body_content =
                                    Self::format_json_smart(&tab.request.body_content);
                                tab.dirty = true;
                            }
                            if ui
                                .button("📦 Minificar")
                                .on_hover_text("Compactar JSON en una sola línea")
                                .clicked()
                            {
                                if let Ok(val) = serde_json::from_str::<serde_json::Value>(
                                    &tab.request.body_content,
                                ) {
                                    if let Ok(compact) = serde_json::to_string(&val) {
                                        tab.request.body_content = compact;
                                        tab.dirty = true;
                                    }
                                }
                            }

                            let trimmed_body = tab.request.body_content.trim();
                            if !trimmed_body.is_empty() {
                                let interpolated_body =
                                    interpolate_vars(trimmed_body, active_env.as_ref());
                                match serde_json::from_str::<serde_json::Value>(&interpolated_body)
                                {
                                    Ok(_) => {
                                        ui.colored_label(
                                            palette.green,
                                            egui::RichText::new("✅ JSON Válido").strong(),
                                        );
                                    }
                                    Err(err) => {
                                        ui.colored_label(
                                            palette.red,
                                            egui::RichText::new(format!(
                                                "⚠ Sintaxis JSON (Línea {}, Col {})",
                                                err.line(),
                                                err.column()
                                            ))
                                            .strong(),
                                        )
                                        .on_hover_text(err.to_string());
                                    }
                                }
                            }
                        }
                    });
                    ui.add_space(6.0);

                    match tab.request.body_type {
                        BodyType::None => {
                            ui.label("Esta petición no envía cuerpo (Body).");
                        }
                        BodyType::Json => {
                            egui::ScrollArea::vertical()
                                .id_salt("req_body_json_scroll")
                                .max_height(185.0)
                                .show(ui, |ui| {
                                    let line_count =
                                        tab.request.body_content.lines().count().max(1);
                                    let line_numbers: String = (1..=line_count)
                                        .map(|n| format!("{:>3}", n))
                                        .collect::<Vec<_>>()
                                        .join("\n");

                                    ui.horizontal_top(|ui| {
                                        ui.add(
                                            egui::Label::new(
                                                egui::RichText::new(line_numbers)
                                                    .font(egui::FontId::monospace(13.5))
                                                    .color(palette.overlay),
                                            )
                                            .selectable(false),
                                        );

                                        let mut json_layouter =
                                            |ui: &egui::Ui, string: &str, wrap_width: f32| {
                                                let layout_job = Self::build_json_editor_layout_job(
                                                    string, &palette, wrap_width,
                                                );
                                                ui.fonts(|f| f.layout_job(layout_job))
                                            };

                                        let edit_resp = ui.add(
                                            egui::TextEdit::multiline(
                                                &mut tab.request.body_content,
                                            )
                                            .code_editor()
                                            .desired_rows(8)
                                            .desired_width(f32::INFINITY)
                                            .layouter(&mut json_layouter),
                                        );
                                        if edit_resp.changed() {
                                            tab.dirty = true;
                                        }
                                    });
                                });
                        }
                        BodyType::RawText => {
                            egui::ScrollArea::vertical()
                                .id_salt("req_body_raw_scroll")
                                .max_height(185.0)
                                .show(ui, |ui| {
                                    let edit_resp = ui.add(
                                        egui::TextEdit::multiline(&mut tab.request.body_content)
                                            .code_editor()
                                            .desired_rows(8)
                                            .desired_width(f32::INFINITY),
                                    );
                                    if edit_resp.changed() {
                                        tab.dirty = true;
                                    }
                                });
                        }
                        BodyType::FormUrlEncoded => {
                            Self::render_key_value_table(
                                ui,
                                "form_urlencoded_table",
                                &mut tab.request.form_data,
                                palette.text,
                                false,
                            );
                        }
                        BodyType::MultipartForm => {
                            Self::render_multipart_table(
                                ui,
                                &mut tab.request.multipart_data,
                                palette.text,
                            );
                        }
                    }
                }
                RequestTab::TestsAndVars => {
                    let tab = &mut self.tabs[self.active_tab_idx];
                    egui::ScrollArea::vertical()
                        .id_salt("tests_and_vars_scroll")
                        .max_height(210.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(
                                    "✅ Aserciones (Tests automáticos al recibir respuesta):",
                                )
                                .strong()
                                .color(palette.mauve),
                            );
                            let mut rm_assert = None;
                            for (idx, a) in tab.request.assertions.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut a.enabled, "");
                                    egui::ComboBox::from_id_salt(format!("assert_kind_{}", idx))
                                        .selected_text(a.kind.label())
                                        .show_ui(ui, |ui| {
                                            for k in AssertionKind::all() {
                                                ui.selectable_value(&mut a.kind, *k, k.label());
                                            }
                                        });
                                    if matches!(
                                        a.kind,
                                        AssertionKind::JsonFieldEquals
                                            | AssertionKind::HeaderContains
                                    ) {
                                        ui.add(
                                            egui::TextEdit::singleline(&mut a.target)
                                                .hint_text(
                                                    if a.kind == AssertionKind::JsonFieldEquals {
                                                        "Ruta JSON (ej. userId o data.id)"
                                                    } else {
                                                        "Nombre Header (ej. content-type)"
                                                    },
                                                )
                                                .desired_width(170.0),
                                        );
                                    }
                                    ui.add(
                                        egui::TextEdit::singleline(&mut a.expected)
                                            .hint_text("Valor esperado (ej. 200)")
                                            .desired_width(160.0),
                                    );
                                    if ui.small_button("🗑").clicked() {
                                        rm_assert = Some(idx);
                                    }
                                });
                            }
                            if let Some(i) = rm_assert {
                                tab.request.assertions.remove(i);
                            }
                            if ui.small_button("+ Agregar Test").clicked() {
                                tab.request
                                    .assertions
                                    .push(TestAssertion::default_status_200());
                            }

                            ui.separator();
                            ui.label(
                                egui::RichText::new(
                                    "🔄 Capturar Variables de la Respuesta JSON hacia el Entorno Activo:",
                                )
                                .strong()
                                .color(palette.sapphire),
                            );
                            let mut rm_ext = None;
                            for (idx, ex) in tab.request.extractions.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.checkbox(&mut ex.enabled, "");
                                    ui.label("Ruta JSON:");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut ex.json_path)
                                            .hint_text("Ej. token o data.access_token")
                                            .desired_width(170.0),
                                    );
                                    ui.label("➡ Guardar en {{variable}}:");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut ex.env_var_name)
                                            .hint_text("Ej. api_token")
                                            .desired_width(150.0),
                                    );
                                    if ui.small_button("🗑").clicked() {
                                        rm_ext = Some(idx);
                                    }
                                });
                            }
                            if let Some(i) = rm_ext {
                                tab.request.extractions.remove(i);
                            }
                            if ui.small_button("+ Agregar Captura de Variable").clicked() {
                                tab.request.extractions.push(VarExtraction::empty());
                            }
                        });
                }
                RequestTab::CodeGen => {
                    let snippet = generate_code_snippet(
                        &self.tabs[self.active_tab_idx].request,
                        active_env.as_ref(),
                        self.selected_code_lang,
                    );
                    ui.horizontal(|ui| {
                        for lang in CodeLang::all() {
                            ui.selectable_value(&mut self.selected_code_lang, *lang, lang.label());
                        }
                        if ui.button("📋 Copiar Código").clicked() {
                            ui.output_mut(|o| o.copied_text = snippet.clone());
                            self.status_message =
                                Some("📋 Código copiado al portapapeles.".to_string());
                        }
                    });

                    egui::ScrollArea::vertical()
                        .id_salt("code_snippet_scroll")
                        .max_height(120.0)
                        .show(ui, |ui| {
                            let mut code_view = snippet.as_str();
                            ui.add(
                                egui::TextEdit::multiline(&mut code_view)
                                    .code_editor()
                                    .desired_rows(5)
                                    .desired_width(f32::INFINITY),
                            );
                        });

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("📥 Importar desde cURL:")
                                .strong()
                                .color(palette.mauve),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.curl_import_input)
                                .hint_text(
                                    "Pega aquí tu comando: curl -X POST https://api.com -H \"...\" -d '{...}'",
                                )
                                .desired_width(ui.available_width() - 180.0),
                        );
                        if ui.button("⚡ Convertir a Petición").clicked() {
                            match parse_curl_command(&self.curl_import_input) {
                                Ok(parsed_req) => {
                                    self.tabs[self.active_tab_idx].request = parsed_req;
                                    self.tabs[self.active_tab_idx].dirty = true;
                                    self.curl_import_input.clear();
                                    self.status_message = Some(
                                        "✅ Comando cURL importado en la pestaña actual."
                                            .to_string(),
                                    );
                                }
                                Err(err) => {
                                    self.status_message = Some(format!("❌ {}", err));
                                }
                            }
                        }
                    });
                }
                RequestTab::Settings => {
                    let settings = &mut self.tabs[self.active_tab_idx].request.settings;
                    ui.horizontal(|ui| {
                        ui.label("⏱ Timeout máximo de la petición (segundos):");
                        ui.add(egui::DragValue::new(&mut settings.timeout_seconds).range(1..=300));
                    });
                    ui.checkbox(
                        &mut settings.follow_redirects,
                        "↪ Seguir redirecciones HTTP automáticamente (máximo 10 saltos)",
                    );
                    ui.checkbox(
                        &mut settings.accept_invalid_certs,
                        "🔓 Aceptar certificados SSL autofirmados o inválidos (útil en desarrollo local)",
                    );
                }
                RequestTab::EnvPreview => {
                    let interpolated = interpolate_vars(
                        &self.tabs[self.active_tab_idx].request.url,
                        active_env.as_ref(),
                    );
                    let resolved_url = normalize_url(&interpolated);
                    ui.label("URL final (con protocolo autocompletado y variables de entorno):");
                    ui.code(resolved_url);
                }
            });

            ui.add_space(6.0);
            ui.separator();

            // Panel Inferior: Respuesta HTTP
            let active_resp = self.tabs[self.active_tab_idx].response.as_ref();
            let active_is_loading = self.tabs[self.active_tab_idx].is_loading;

            ui.horizontal_wrapped(|ui| {
                ui.heading("📥 Respuesta");

                if let Some(resp) = active_resp {
                    ui.separator();
                    let status_color = if (200..300).contains(&resp.status_code) {
                        palette.green
                    } else if (300..400).contains(&resp.status_code) {
                        palette.peach
                    } else {
                        palette.red
                    };

                    ui.colored_label(
                        status_color,
                        egui::RichText::new(format!(
                            "Estado: {} {}",
                            resp.status_code, resp.status_text
                        ))
                        .strong(),
                    );
                    ui.separator();
                    ui.label(format!("⏱ {} ms", resp.elapsed_ms));
                    ui.separator();
                    ui.label(format!("📦 {} bytes", resp.size_bytes));
                    ui.separator();

                    let format_badge = match resp.format {
                        ResponseFormat::Json => ("JSON", palette.sapphire),
                        ResponseFormat::Xml => ("XML", palette.peach),
                        ResponseFormat::Html => ("HTML", palette.mauve),
                        ResponseFormat::Text => ("TEXT", palette.subtext),
                    };
                    ui.colored_label(
                        format_badge.1,
                        egui::RichText::new(format!("[{}]", format_badge.0)).strong(),
                    );

                    if ui.button("📋 Copiar").clicked() {
                        ui.output_mut(|o| o.copied_text = resp.body.clone());
                        self.status_message =
                            Some("📋 Respuesta copiada al portapapeles.".to_string());
                    }

                    if ui.button("💾 Guardar Archivo").clicked() {
                        let ext = match resp.format {
                            ResponseFormat::Json => "json",
                            ResponseFormat::Xml => "xml",
                            ResponseFormat::Html => "html",
                            ResponseFormat::Text => "txt",
                        };
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name(format!("respuesta.{}", ext))
                            .save_file()
                        {
                            let _ = fs::write(&path, &resp.body);
                        }
                    }

                    ui.separator();
                    let search_edit = ui.add(
                        egui::TextEdit::singleline(&mut self.response_search_query)
                            .hint_text("🔍 Buscar en respuesta (Ctrl+F)...")
                            .desired_width(180.0),
                    );
                    if self.focus_search_next_frame {
                        search_edit.request_focus();
                        self.focus_search_next_frame = false;
                    }
                    if !self.response_search_query.trim().is_empty() {
                        let q = self.response_search_query.trim().to_lowercase();
                        let matches_count = resp.body.to_lowercase().matches(&q).count();
                        ui.colored_label(
                            palette.peach,
                            format!("({} coincidencias)", matches_count),
                        );
                        ui.checkbox(&mut self.response_filter_matching_lines, "Filtrar líneas");
                    }
                }
            });

            ui.add_space(4.0);

            if let Some(resp) = active_resp {
                if let Some(err) = &resp.error {
                    ui.colored_label(palette.red, format!("❌ {}", err));
                }

                let passed_tests = resp.test_results.iter().filter(|t| t.passed).count();
                let total_tests = resp.test_results.len();

                ui.horizontal_wrapped(|ui| {
                    let pretty_label = match resp.format {
                        ResponseFormat::Json => "✨ Pretty (JSON)",
                        ResponseFormat::Xml => "✨ Pretty (XML)",
                        ResponseFormat::Html => "✨ Pretty (HTML)",
                        ResponseFormat::Text => "📄 Pretty (Texto)",
                    };
                    ui.selectable_value(&mut self.resp_tab, ResponseTab::Pretty, pretty_label);
                    ui.selectable_value(
                        &mut self.resp_tab,
                        ResponseTab::Raw,
                        "📝 Raw (Sin formato)",
                    );
                    if resp.format == ResponseFormat::Html || !resp.raw_body.is_empty() {
                        ui.selectable_value(
                            &mut self.resp_tab,
                            ResponseTab::Preview,
                            "👁 Visualizar (HTML Preview)",
                        );
                    }
                    ui.selectable_value(
                        &mut self.resp_tab,
                        ResponseTab::Headers,
                        format!("📋 Headers ({})", resp.headers.len()),
                    );
                    ui.selectable_value(
                        &mut self.resp_tab,
                        ResponseTab::Cookies,
                        format!("🍪 Cookies ({})", resp.cookies.len()),
                    );
                    let tests_label = if total_tests > 0 {
                        format!("🧪 Tests ({}/{})", passed_tests, total_tests)
                    } else {
                        "🧪 Tests (0)".to_string()
                    };
                    ui.selectable_value(&mut self.resp_tab, ResponseTab::Tests, tests_label);
                });
                ui.separator();

                match self.resp_tab {
                    ResponseTab::Pretty => {
                        egui::ScrollArea::vertical()
                            .id_salt("resp_pretty_scroll")
                            .show(ui, |ui| {
                                Self::render_syntax_highlighted_code(
                                    ui,
                                    &resp.body,
                                    resp.format,
                                    &self.response_search_query,
                                    self.response_filter_matching_lines,
                                    &palette,
                                );
                            });
                    }
                    ResponseTab::Raw => {
                        egui::ScrollArea::vertical()
                            .id_salt("resp_raw_scroll")
                            .show(ui, |ui| {
                                let raw_str = if resp.raw_body.is_empty() {
                                    resp.body.as_str()
                                } else {
                                    resp.raw_body.as_str()
                                };
                                let mut body_view = raw_str;
                                ui.add(
                                    egui::TextEdit::multiline(&mut body_view)
                                        .code_editor()
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(16),
                                );
                            });
                    }
                    ResponseTab::Preview => {
                        egui::ScrollArea::vertical()
                            .id_salt("resp_html_preview_scroll")
                            .show(ui, |ui| {
                                let html_source = if resp.raw_body.is_empty() {
                                    &resp.body
                                } else {
                                    &resp.raw_body
                                };
                                Self::render_html_preview(ui, html_source, &palette);
                            });
                    }
                    ResponseTab::Headers => {
                        egui::ScrollArea::vertical()
                            .id_salt("resp_headers_scroll")
                            .show(ui, |ui| {
                                egui::Grid::new("resp_headers_grid")
                                    .striped(true)
                                    .num_columns(2)
                                    .spacing([16.0, 6.0])
                                    .show(ui, |ui| {
                                        for (k, v) in &resp.headers {
                                            Self::col_header(ui, k, palette.text);
                                            ui.monospace(v);
                                            ui.end_row();
                                        }
                                    });
                            });
                    }
                    ResponseTab::Cookies => {
                        if resp.cookies.is_empty() {
                            ui.label("El servidor no envió cabeceras Set-Cookie en esta respuesta.");
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt("resp_cookies_scroll")
                                .show(ui, |ui| {
                                    egui::Grid::new("resp_cookies_grid")
                                        .striped(true)
                                        .num_columns(2)
                                        .spacing([16.0, 6.0])
                                        .show(ui, |ui| {
                                            Self::col_header(ui, "Cookie", palette.text);
                                            Self::col_header(ui, "Valor", palette.text);
                                            ui.end_row();
                                            for (ck, cv) in &resp.cookies {
                                                ui.colored_label(palette.sapphire, ck);
                                                ui.monospace(cv);
                                                ui.end_row();
                                            }
                                        });
                                });
                        }
                    }
                    ResponseTab::Tests => {
                        egui::ScrollArea::vertical()
                            .id_salt("resp_tests_scroll")
                            .show(ui, |ui| {
                                if resp.test_results.is_empty() {
                                    ui.label("No hay tests configurados para esta petición.");
                                } else {
                                    for tr in &resp.test_results {
                                        ui.horizontal(|ui| {
                                            if tr.passed {
                                                ui.colored_label(
                                                    palette.green,
                                                    egui::RichText::new("✅ PASS").strong(),
                                                );
                                            } else {
                                                ui.colored_label(
                                                    palette.red,
                                                    egui::RichText::new("❌ FAIL").strong(),
                                                );
                                            }
                                            ui.strong(&tr.name);
                                            ui.label("—");
                                            ui.label(
                                                egui::RichText::new(&tr.detail)
                                                    .color(palette.subtext),
                                            );
                                        });
                                    }
                                }

                                if !resp.extracted_vars.is_empty() {
                                    ui.separator();
                                    ui.label(
                                        egui::RichText::new(
                                            "🔄 Variables capturadas y guardadas en el entorno activo:",
                                        )
                                        .strong()
                                        .color(palette.sapphire),
                                    );
                                    for (k, v) in &resp.extracted_vars {
                                        ui.horizontal(|ui| {
                                            ui.code(format!("{{{{{}}}}}", k));
                                            ui.label("=");
                                            ui.monospace(v);
                                        });
                                    }
                                }
                            });
                    }
                }
            } else if active_is_loading {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.spinner();
                    ui.label("Esperando respuesta del servidor...");
                });
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        egui::RichText::new(
                            "Configura tu petición arriba y presiona 'Enviar' (o Ctrl+Enter)",
                        )
                        .color(palette.overlay),
                    );
                });
            }
        });
    }
}
