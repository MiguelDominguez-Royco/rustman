use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    HEAD,
    OPTIONS,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    #[serde(alias = "TokyoNightDark")]
    CatppuccinMocha,
    CatppuccinMacchiato,
    CatppuccinFrappe,
    #[serde(alias = "TokyoNightDay")]
    CatppuccinLatte,
}

impl ThemeMode {
    pub fn all() -> &'static [ThemeMode] {
        &[
            ThemeMode::CatppuccinMocha,
            ThemeMode::CatppuccinMacchiato,
            ThemeMode::CatppuccinFrappe,
            ThemeMode::CatppuccinLatte,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            ThemeMode::CatppuccinMocha => "🌙 Catppuccin Mocha (Oscuro)",
            ThemeMode::CatppuccinMacchiato => "☕ Catppuccin Macchiato (Oscuro Medio)",
            ThemeMode::CatppuccinFrappe => "🌆 Catppuccin Frappé (Oscuro Suave)",
            ThemeMode::CatppuccinLatte => "☀ Catppuccin Latte (Claro)",
        }
    }

    pub fn is_light(&self) -> bool {
        matches!(self, ThemeMode::CatppuccinLatte)
    }
}

impl HttpMethod {
    pub fn all() -> &'static [HttpMethod] {
        &[
            HttpMethod::GET,
            HttpMethod::POST,
            HttpMethod::PUT,
            HttpMethod::DELETE,
            HttpMethod::PATCH,
            HttpMethod::HEAD,
            HttpMethod::OPTIONS,
        ]
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
            HttpMethod::PUT => "PUT",
            HttpMethod::DELETE => "DELETE",
            HttpMethod::PATCH => "PATCH",
            HttpMethod::HEAD => "HEAD",
            HttpMethod::OPTIONS => "OPTIONS",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_uppercase().as_str() {
            "POST" => HttpMethod::POST,
            "PUT" => HttpMethod::PUT,
            "DELETE" => HttpMethod::DELETE,
            "PATCH" => HttpMethod::PATCH,
            "HEAD" => HttpMethod::HEAD,
            "OPTIONS" => HttpMethod::OPTIONS,
            _ => HttpMethod::GET,
        }
    }

    pub fn color(&self, theme: ThemeMode) -> egui::Color32 {
        if theme.is_light() {
            // Catppuccin Latte (Alto contraste en claro)
            match self {
                HttpMethod::GET => egui::Color32::from_rgb(30, 102, 245),     // #1e66f5 (Blue)
                HttpMethod::POST => egui::Color32::from_rgb(64, 160, 43),     // #40a02b (Green)
                HttpMethod::PUT => egui::Color32::from_rgb(254, 100, 11),     // #fe640b (Peach)
                HttpMethod::DELETE => egui::Color32::from_rgb(210, 15, 57),   // #d20f39 (Red)
                HttpMethod::PATCH => egui::Color32::from_rgb(23, 146, 153),   // #179299 (Teal)
                HttpMethod::HEAD => egui::Color32::from_rgb(136, 57, 239),    // #8839ef (Mauve)
                HttpMethod::OPTIONS => egui::Color32::from_rgb(4, 165, 229),  // #04a5e5 (Sky)
            }
        } else {
            // Catppuccin Mocha / Macchiato / Frappé (Alto contraste en oscuro)
            match self {
                HttpMethod::GET => egui::Color32::from_rgb(137, 180, 250),    // #89b4fa (Blue)
                HttpMethod::POST => egui::Color32::from_rgb(166, 227, 161),   // #a6e3a1 (Green)
                HttpMethod::PUT => egui::Color32::from_rgb(250, 179, 135),    // #fab387 (Peach)
                HttpMethod::DELETE => egui::Color32::from_rgb(243, 139, 168), // #f38ba8 (Red)
                HttpMethod::PATCH => egui::Color32::from_rgb(148, 226, 213),  // #94e2d5 (Teal)
                HttpMethod::HEAD => egui::Color32::from_rgb(203, 166, 247),   // #cba6f7 (Mauve)
                HttpMethod::OPTIONS => egui::Color32::from_rgb(137, 220, 235),// #89dceb (Sky)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyValue {
    pub enabled: bool,
    pub key: String,
    pub value: String,
    pub description: String,
}

impl KeyValue {
    pub fn new(key: &str, value: &str) -> Self {
        Self {
            enabled: true,
            key: key.to_string(),
            value: value.to_string(),
            description: String::new(),
        }
    }

    pub fn empty() -> Self {
        Self::new("", "")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthConfig {
    None,
    Bearer {
        token: String,
    },
    Basic {
        username: String,
        password: String,
    },
    ApiKey {
        key: String,
        value: String,
        in_query: bool, // false = Header, true = Query Param
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyType {
    None,
    Json,
    RawText,
    FormUrlEncoded,
    MultipartForm,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MultipartKind {
    #[default]
    Text,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultipartField {
    pub enabled: bool,
    pub key: String,
    pub kind: MultipartKind,
    pub value: String, // Valor de texto o ruta absoluta del archivo
    pub description: String,
}

impl MultipartField {
    pub fn empty() -> Self {
        Self {
            enabled: true,
            key: String::new(),
            kind: MultipartKind::Text,
            value: String::new(),
            description: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AssertionKind {
    #[default]
    StatusEquals,
    TimeLessThanMs,
    BodyContains,
    JsonFieldEquals,
    HeaderContains,
}

impl AssertionKind {
    pub fn all() -> &'static [AssertionKind] {
        &[
            AssertionKind::StatusEquals,
            AssertionKind::TimeLessThanMs,
            AssertionKind::BodyContains,
            AssertionKind::JsonFieldEquals,
            AssertionKind::HeaderContains,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            AssertionKind::StatusEquals => "Status Code ==",
            AssertionKind::TimeLessThanMs => "Tiempo (ms) <",
            AssertionKind::BodyContains => "Body contiene texto",
            AssertionKind::JsonFieldEquals => "Campo JSON ==",
            AssertionKind::HeaderContains => "Header contiene",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestAssertion {
    pub enabled: bool,
    pub kind: AssertionKind,
    pub target: String, // Ruta JSON o nombre de Header (opcional según el tipo)
    pub expected: String,
}

impl TestAssertion {
    pub fn default_status_200() -> Self {
        Self {
            enabled: true,
            kind: AssertionKind::StatusEquals,
            target: String::new(),
            expected: "200".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VarExtraction {
    pub enabled: bool,
    pub json_path: String,    // Ej. "id" o "data.token"
    pub env_var_name: String, // Ej. "api_token"
}

impl VarExtraction {
    pub fn empty() -> Self {
        Self {
            enabled: true,
            json_path: String::new(),
            env_var_name: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestSettings {
    pub timeout_seconds: u64,
    pub follow_redirects: bool,
    pub accept_invalid_certs: bool,
}

impl Default for RequestSettings {
    fn default() -> Self {
        Self {
            timeout_seconds: 30,
            follow_redirects: true,
            accept_invalid_certs: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeLang {
    Curl,
    JavaScriptFetch,
    PythonRequests,
    RustReqwest,
    CSharpHttpClient,
}

impl CodeLang {
    pub fn all() -> &'static [CodeLang] {
        &[
            CodeLang::Curl,
            CodeLang::JavaScriptFetch,
            CodeLang::PythonRequests,
            CodeLang::RustReqwest,
            CodeLang::CSharpHttpClient,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            CodeLang::Curl => "💻 cURL",
            CodeLang::JavaScriptFetch => "🟨 JavaScript (fetch)",
            CodeLang::PythonRequests => "🐍 Python (requests)",
            CodeLang::RustReqwest => "🦀 Rust (reqwest)",
            CodeLang::CSharpHttpClient => "🟣 C# (HttpClient)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestItem {
    pub id: Uuid,
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub query_params: Vec<KeyValue>,
    pub headers: Vec<KeyValue>,
    pub auth: AuthConfig,
    pub body_type: BodyType,
    pub body_content: String,
    pub form_data: Vec<KeyValue>,
    #[serde(default = "default_multipart_fields")]
    pub multipart_data: Vec<MultipartField>,
    #[serde(default = "default_assertions")]
    pub assertions: Vec<TestAssertion>,
    #[serde(default)]
    pub extractions: Vec<VarExtraction>,
    #[serde(default)]
    pub settings: RequestSettings,
}

fn default_multipart_fields() -> Vec<MultipartField> {
    vec![MultipartField::empty()]
}

fn default_assertions() -> Vec<TestAssertion> {
    vec![TestAssertion::default_status_200()]
}

impl Default for HttpRequestItem {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Nueva Petición".to_string(),
            method: HttpMethod::GET,
            url: "https://jsonplaceholder.typicode.com/posts/1".to_string(),
            query_params: vec![KeyValue::empty()],
            headers: vec![
                KeyValue::new("Accept", "application/json"),
                KeyValue::new("User-Agent", "Rustman/0.2.0"),
            ],
            auth: AuthConfig::None,
            body_type: BodyType::Json,
            body_content: "{\n  \"title\": \"Hola desde Rustman\",\n  \"body\": \"Contenido de prueba\",\n  \"userId\": 1\n}".to_string(),
            form_data: vec![KeyValue::empty()],
            multipart_data: vec![MultipartField::empty()],
            assertions: vec![TestAssertion::default_status_200()],
            extractions: Vec::new(),
            settings: RequestSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ResponseFormat {
    #[default]
    Text,
    Json,
    Xml,
    Html,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpResponseData {
    pub status_code: u16,
    pub status_text: String,
    pub elapsed_ms: u128,
    pub size_bytes: usize,
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub cookies: Vec<(String, String)>,
    pub raw_body: String,
    pub body: String,
    #[serde(default)]
    pub format: ResponseFormat,
    pub is_json: bool,
    pub error: Option<String>,
    #[serde(default)]
    pub test_results: Vec<TestResult>,
    #[serde(default)]
    pub extracted_vars: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Local>,
    pub request: HttpRequestItem,
    pub status_code: Option<u16>,
    pub elapsed_ms: Option<u128>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: Uuid,
    pub name: String,
    pub requests: Vec<HttpRequestItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    pub id: Uuid,
    pub name: String,
    pub variables: Vec<KeyValue>,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Desarrollo (Local)".to_string(),
            variables: vec![
                KeyValue::new("base_url", "https://jsonplaceholder.typicode.com"),
                KeyValue::new("api_token", "mi_token_secreto_123"),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStorage {
    pub collections: Vec<Collection>,
    pub history: Vec<HistoryEntry>,
    pub environments: Vec<Environment>,
    pub active_env_index: Option<usize>,
    #[serde(default)]
    pub theme: ThemeMode,
    #[serde(default)]
    pub cookies: Vec<KeyValue>,
}

impl Default for AppStorage {
    fn default() -> Self {
        let demo_collection = Collection {
            id: Uuid::new_v4(),
            name: "Ejemplos JSONPlaceholder".to_string(),
            requests: vec![
                HttpRequestItem {
                    id: Uuid::new_v4(),
                    name: "Obtener Posts (GET)".to_string(),
                    method: HttpMethod::GET,
                    url: "{{base_url}}/posts/1".to_string(),
                    ..Default::default()
                },
                HttpRequestItem {
                    id: Uuid::new_v4(),
                    name: "Crear Post (POST)".to_string(),
                    method: HttpMethod::POST,
                    url: "{{base_url}}/posts".to_string(),
                    body_type: BodyType::Json,
                    body_content: "{\n  \"title\": \"Vento Thriller 250\",\n  \"body\": \"Cafe Racer 250cc\",\n  \"userId\": 1\n}".to_string(),
                    assertions: vec![TestAssertion {
                        enabled: true,
                        kind: AssertionKind::StatusEquals,
                        target: String::new(),
                        expected: "201".to_string(),
                    }],
                    ..Default::default()
                },
            ],
        };

        Self {
            collections: vec![demo_collection],
            history: Vec::new(),
            environments: vec![Environment::default()],
            active_env_index: Some(0),
            theme: ThemeMode::CatppuccinMocha,
            cookies: Vec::new(),
        }
    }
}
