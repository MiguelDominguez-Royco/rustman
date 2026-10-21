use crate::models::{
    AssertionKind, AuthConfig, BodyType, CodeLang, Collection, Environment, HttpMethod,
    HttpRequestItem, HttpResponseData, KeyValue, MultipartKind, ResponseFormat, TestAssertion,
    TestResult, VarExtraction,
};
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE, COOKIE};
use reqwest::multipart::{Form, Part};
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Reemplaza variables de entorno con formato {{variable}} en un texto
pub fn interpolate_vars(input: &str, env: Option<&Environment>) -> String {
    let Some(env) = env else {
        return input.to_string();
    };

    let mut result = input.to_string();
    for var in &env.variables {
        if var.enabled && !var.key.trim().is_empty() {
            let pattern = format!("{{{{{}}}}}", var.key.trim());
            result = result.replace(&pattern, &var.value);
        }
    }
    result
}

/// Normaliza la URL al estilo de los navegadores (Chrome) o Postman:
/// Si no tiene `http://` ni `https://`, agrega `http://` para localhost/IPs o `https://` para dominios.
pub fn normalize_url(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let lower = trimmed.to_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return trimmed.to_string();
    }
    if trimmed.starts_with(':') {
        return format!("http://localhost{}", trimmed);
    }
    if lower.starts_with("localhost")
        || lower.starts_with("127.")
        || lower.starts_with("0.0.0.0")
        || lower.starts_with("192.168.")
        || lower.starts_with("10.")
        || lower.starts_with("[::1]")
    {
        format!("http://{}", trimmed)
    } else if trimmed.contains(':') && !trimmed.contains(":443") {
        format!("http://{}", trimmed)
    } else {
        format!("https://{}", trimmed)
    }
}

/// Formatea e indenta código XML o HTML de forma limpia
pub fn format_xml_or_html(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut result = String::with_capacity(trimmed.len() + 256);
    let mut indent: usize = 0;

    let normalized = trimmed.replace(">\r\n<", "><").replace(">\n<", "><");
    let mut tokens = Vec::new();
    let mut current = String::new();

    for c in normalized.chars() {
        if c == '<' && !current.is_empty() {
            tokens.push(current.trim().to_string());
            current = String::new();
        }
        current.push(c);
        if c == '>' {
            tokens.push(current.trim().to_string());
            current = String::new();
        }
    }
    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }

    let void_html_tags = [
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr",
    ];

    for token in tokens {
        if token.is_empty() {
            continue;
        }

        let is_closing = token.starts_with("</");
        let is_declaration_or_comment =
            token.starts_with("<?") || token.starts_with("<!") || token.ends_with("/>");

        let tag_name = if token.starts_with('<') && !is_closing && !is_declaration_or_comment {
            token[1..]
                .split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .next()
                .unwrap_or("")
                .to_lowercase()
        } else {
            String::new()
        };

        let is_void_tag = void_html_tags.contains(&tag_name.as_str());

        if is_closing {
            indent = indent.saturating_sub(1);
        }

        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str(&token);
        result.push('\n');

        if token.starts_with('<') && !is_closing && !is_declaration_or_comment && !is_void_tag {
            indent += 1;
        }
    }

    result.trim_end().to_string()
}

/// Detecta el formato de la respuesta (JSON, XML, HTML o Texto) y lo formatea automáticamente
pub fn detect_and_format_body(
    raw_text: &str,
    headers: &[(String, String)],
) -> (String, ResponseFormat) {
    let content_type = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| v.to_lowercase())
        .unwrap_or_default();

    let trimmed = raw_text.trim();

    if content_type.contains("json") || trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Ok(pretty) = serde_json::to_string_pretty(&json_val) {
                return (pretty, ResponseFormat::Json);
            }
        }
    }

    let lower_start = trimmed.chars().take(300).collect::<String>().to_lowercase();
    if content_type.contains("html")
        || lower_start.starts_with("<!doctype html")
        || lower_start.contains("<html")
        || lower_start.contains("<body")
    {
        return (format_xml_or_html(raw_text), ResponseFormat::Html);
    }

    if content_type.contains("xml")
        || lower_start.starts_with("<?xml")
        || (trimmed.starts_with('<') && trimmed.ends_with('>'))
    {
        return (format_xml_or_html(raw_text), ResponseFormat::Xml);
    }

    (raw_text.to_string(), ResponseFormat::Text)
}

/// Extrae un valor de un `serde_json::Value` usando una ruta con puntos (ej. `data.user.id` o `0.title`)
pub fn extract_json_path(root: &serde_json::Value, path: &str) -> Option<String> {
    let mut current = root;
    for segment in path.trim().split('.') {
        if segment.is_empty() {
            continue;
        }
        match current {
            serde_json::Value::Object(map) => {
                current = map.get(segment)?;
            }
            serde_json::Value::Array(arr) => {
                let idx: usize = segment.parse().ok()?;
                current = arr.get(idx)?;
            }
            _ => return None,
        }
    }
    match current {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Null => Some("null".to_string()),
        other => Some(other.to_string()),
    }
}

/// Evalúa las aserciones/tests configurados en la petición contra la respuesta recibida
fn evaluate_assertions(
    assertions: &[TestAssertion],
    status_code: u16,
    elapsed_ms: u128,
    raw_body: &str,
    headers: &[(String, String)],
    parsed_json: Option<&serde_json::Value>,
) -> Vec<TestResult> {
    let mut results = Vec::new();
    for a in assertions {
        if !a.enabled {
            continue;
        }
        match a.kind {
            AssertionKind::StatusEquals => {
                let expected_code = a.expected.trim().parse::<u16>().unwrap_or(200);
                let passed = status_code == expected_code;
                results.push(TestResult {
                    name: format!("Status Code == {}", expected_code),
                    passed,
                    detail: format!("Recibido: {} (Esperado: {})", status_code, expected_code),
                });
            }
            AssertionKind::TimeLessThanMs => {
                let max_ms = a.expected.trim().parse::<u128>().unwrap_or(1000);
                let passed = elapsed_ms < max_ms;
                results.push(TestResult {
                    name: format!("Tiempo de respuesta < {} ms", max_ms),
                    passed,
                    detail: format!("Tiempo real: {} ms", elapsed_ms),
                });
            }
            AssertionKind::BodyContains => {
                let needle = a.expected.trim();
                let passed = raw_body.contains(needle);
                results.push(TestResult {
                    name: format!("Body contiene '{}'", needle),
                    passed,
                    detail: if passed {
                        "Texto encontrado en el cuerpo de la respuesta".to_string()
                    } else {
                        "No se encontró el texto en el cuerpo".to_string()
                    },
                });
            }
            AssertionKind::JsonFieldEquals => {
                let path = a.target.trim();
                let expected_val = a.expected.trim();
                if let Some(json) = parsed_json {
                    if let Some(actual) = extract_json_path(json, path) {
                        let passed = actual == expected_val;
                        results.push(TestResult {
                            name: format!("JSON '{}' == '{}'", path, expected_val),
                            passed,
                            detail: format!("Valor actual: '{}' (Esperado: '{}')", actual, expected_val),
                        });
                    } else {
                        results.push(TestResult {
                            name: format!("JSON '{}' == '{}'", path, expected_val),
                            passed: false,
                            detail: format!("Ruta JSON '{}' no encontrada", path),
                        });
                    }
                } else {
                    results.push(TestResult {
                        name: format!("JSON '{}' == '{}'", path, expected_val),
                        passed: false,
                        detail: "La respuesta no es un JSON válido".to_string(),
                    });
                }
            }
            AssertionKind::HeaderContains => {
                let header_key = a.target.trim().to_lowercase();
                let expected_sub = a.expected.trim().to_lowercase();
                let found = headers.iter().find(|(k, _)| {
                    header_key.is_empty() || k.to_lowercase() == header_key
                });
                let passed = headers.iter().any(|(k, v)| {
                    (header_key.is_empty() || k.to_lowercase().contains(&header_key))
                        && v.to_lowercase().contains(&expected_sub)
                });
                results.push(TestResult {
                    name: format!("Header '{}' contiene '{}'", a.target, a.expected),
                    passed,
                    detail: match found {
                        Some((k, v)) => format!("Header {}: {}", k, v),
                        None => "Header no presente en la respuesta".to_string(),
                    },
                });
            }
        }
    }
    results
}

/// Evalúa las extracciones de variables post-request desde un JSON
fn evaluate_extractions(
    extractions: &[VarExtraction],
    parsed_json: Option<&serde_json::Value>,
) -> Vec<(String, String)> {
    let mut extracted = Vec::new();
    let Some(json) = parsed_json else {
        return extracted;
    };
    for ex in extractions {
        if !ex.enabled || ex.json_path.trim().is_empty() || ex.env_var_name.trim().is_empty() {
            continue;
        }
        if let Some(val) = extract_json_path(json, ex.json_path.trim()) {
            extracted.push((ex.env_var_name.trim().to_string(), val));
        }
    }
    extracted
}

/// Ejecuta una petición HTTP asíncrona usando reqwest y devuelve los detalles de respuesta
pub async fn send_http_request(
    req: HttpRequestItem,
    env: Option<Environment>,
    stored_cookies: Vec<KeyValue>,
) -> HttpResponseData {
    let env_ref = env.as_ref();
    let interpolated_url = interpolate_vars(req.url.trim(), env_ref);
    let raw_url = normalize_url(&interpolated_url);

    if raw_url.is_empty() {
        return HttpResponseData {
            status_code: 0,
            status_text: "URL Vacía".to_string(),
            elapsed_ms: 0,
            size_bytes: 0,
            headers: vec![],
            cookies: vec![],
            raw_body: String::new(),
            body: String::new(),
            format: ResponseFormat::Text,
            is_json: false,
            error: Some("Por favor ingresa una URL válida.".to_string()),
            test_results: vec![],
            extracted_vars: vec![],
        };
    }

    let redirect_policy = if req.settings.follow_redirects {
        reqwest::redirect::Policy::limited(10)
    } else {
        reqwest::redirect::Policy::none()
    };

    let timeout_secs = req.settings.timeout_seconds.max(1);

    let client = match reqwest::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(timeout_secs))
        .redirect(redirect_policy)
        .danger_accept_invalid_certs(req.settings.accept_invalid_certs)
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return HttpResponseData {
                status_code: 0,
                status_text: "Error de Cliente".to_string(),
                elapsed_ms: 0,
                size_bytes: 0,
                headers: vec![],
                cookies: vec![],
                raw_body: String::new(),
                body: String::new(),
                format: ResponseFormat::Text,
                is_json: false,
                error: Some(format!("No se pudo inicializar el cliente HTTP: {}", e)),
                test_results: vec![],
                extracted_vars: vec![],
            };
        }
    };

    let method = match req.method {
        HttpMethod::GET => reqwest::Method::GET,
        HttpMethod::POST => reqwest::Method::POST,
        HttpMethod::PUT => reqwest::Method::PUT,
        HttpMethod::DELETE => reqwest::Method::DELETE,
        HttpMethod::PATCH => reqwest::Method::PATCH,
        HttpMethod::HEAD => reqwest::Method::HEAD,
        HttpMethod::OPTIONS => reqwest::Method::OPTIONS,
    };

    // Construir URL con Query Params sin duplicar si la barra de URL ya tiene ?param=val
    let base_url_without_query = raw_url.split('?').next().unwrap_or(&raw_url).to_string();
    let mut query_pairs: Vec<(String, String)> = Vec::new();
    for param in &req.query_params {
        if param.enabled && !param.key.trim().is_empty() {
            let k = interpolate_vars(param.key.trim(), env_ref);
            let v = interpolate_vars(&param.value, env_ref);
            query_pairs.push((k, v));
        }
    }

    if let AuthConfig::ApiKey {
        ref key,
        ref value,
        in_query: true,
    } = req.auth
    {
        if !key.trim().is_empty() {
            query_pairs.push((
                interpolate_vars(key.trim(), env_ref),
                interpolate_vars(value, env_ref),
            ));
        }
    }

    let mut request_builder = client.request(method, &base_url_without_query);
    if !query_pairs.is_empty() {
        request_builder = request_builder.query(&query_pairs);
    }

    // Construir Headers
    let mut header_map = HeaderMap::new();
    for h in &req.headers {
        if h.enabled && !h.key.trim().is_empty() {
            let k = interpolate_vars(h.key.trim(), env_ref);
            let v = interpolate_vars(&h.value, env_ref);
            if let (Ok(name), Ok(val)) = (HeaderName::from_str(&k), HeaderValue::from_str(&v)) {
                header_map.insert(name, val);
            }
        }
    }

    // Inyectar Cookies guardadas en el Cookie Jar
    let cookie_pairs: Vec<String> = stored_cookies
        .iter()
        .filter(|c| c.enabled && !c.key.trim().is_empty())
        .map(|c| format!("{}={}", c.key.trim(), c.value.trim()))
        .collect();
    if !cookie_pairs.is_empty() && !header_map.contains_key(COOKIE) {
        if let Ok(val) = HeaderValue::from_str(&cookie_pairs.join("; ")) {
            header_map.insert(COOKIE, val);
        }
    }

    // Aplicar Autenticación en Headers
    match &req.auth {
        AuthConfig::None => {}
        AuthConfig::Bearer { token } => {
            let t = interpolate_vars(token.trim(), env_ref);
            if !t.is_empty() {
                if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", t)) {
                    header_map.insert(AUTHORIZATION, val);
                }
            }
        }
        AuthConfig::Basic { username, password } => {
            let u = interpolate_vars(username, env_ref);
            let p = interpolate_vars(password, env_ref);
            let credentials = format!("{}:{}", u, p);
            let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
            if let Ok(val) = HeaderValue::from_str(&format!("Basic {}", encoded)) {
                header_map.insert(AUTHORIZATION, val);
            }
        }
        AuthConfig::ApiKey {
            key,
            value,
            in_query: false,
        } => {
            let k = interpolate_vars(key.trim(), env_ref);
            let v = interpolate_vars(value, env_ref);
            if let (Ok(name), Ok(val)) = (HeaderName::from_str(&k), HeaderValue::from_str(&v)) {
                header_map.insert(name, val);
            }
        }
        _ => {}
    }

    // Configurar Body (no adjuntar body en peticiones GET o HEAD para evitar rechazos de servidores estrictos)
    if !matches!(req.method, HttpMethod::GET | HttpMethod::HEAD) {
        match req.body_type {
            BodyType::None => {}
            BodyType::Json => {
                let body_str = interpolate_vars(&req.body_content, env_ref);
                if !header_map.contains_key(CONTENT_TYPE) {
                    header_map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                }
                request_builder = request_builder.body(body_str);
            }
            BodyType::RawText => {
                let body_str = interpolate_vars(&req.body_content, env_ref);
                if !header_map.contains_key(CONTENT_TYPE) {
                    header_map.insert(
                        CONTENT_TYPE,
                        HeaderValue::from_static("text/plain; charset=utf-8"),
                    );
                }
                request_builder = request_builder.body(body_str);
            }
            BodyType::FormUrlEncoded => {
                let mut form_pairs: Vec<(String, String)> = Vec::new();
                for item in &req.form_data {
                    if item.enabled && !item.key.trim().is_empty() {
                        form_pairs.push((
                            interpolate_vars(item.key.trim(), env_ref),
                            interpolate_vars(&item.value, env_ref),
                        ));
                    }
                }
                request_builder = request_builder.form(&form_pairs);
            }
            BodyType::MultipartForm => {
                let mut form = Form::new();
                for item in &req.multipart_data {
                    if !item.enabled || item.key.trim().is_empty() {
                        continue;
                    }
                    let field_name = interpolate_vars(item.key.trim(), env_ref);
                    match item.kind {
                        MultipartKind::Text => {
                            let text_val = interpolate_vars(&item.value, env_ref);
                            form = form.text(field_name, text_val);
                        }
                        MultipartKind::File => {
                            let file_path_str = interpolate_vars(item.value.trim(), env_ref);
                            let path = Path::new(&file_path_str);
                            match std::fs::read(path) {
                                Ok(bytes) => {
                                    let file_name = path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .unwrap_or("archivo.bin")
                                        .to_string();
                                    let mime = mime_guess::from_path(path)
                                        .first_or_octet_stream()
                                        .essence_str()
                                        .to_string();

                                    let part = Part::bytes(bytes)
                                        .file_name(file_name)
                                        .mime_str(&mime)
                                        .unwrap_or_else(|_| Part::bytes(Vec::new()));
                                    form = form.part(field_name, part);
                                }
                                Err(e) => {
                                    return HttpResponseData {
                                        status_code: 0,
                                        status_text: "Error de Archivo".to_string(),
                                        elapsed_ms: 0,
                                        size_bytes: 0,
                                        headers: vec![],
                                        cookies: vec![],
                                        raw_body: String::new(),
                                        body: String::new(),
                                        format: ResponseFormat::Text,
                                        is_json: false,
                                        error: Some(format!(
                                            "No se pudo leer el archivo '{}' para el campo '{}': {}",
                                            file_path_str, field_name, e
                                        )),
                                        test_results: vec![],
                                        extracted_vars: vec![],
                                    };
                                }
                            }
                        }
                    }
                }
                header_map.remove(CONTENT_TYPE);
                request_builder = request_builder.multipart(form);
            }
        }
    }

    request_builder = request_builder.headers(header_map);

    let start_time = Instant::now();
    let response_result = request_builder.send().await;
    let elapsed_ms = start_time.elapsed().as_millis();

    match response_result {
        Ok(resp) => {
            let status = resp.status();
            let status_code = status.as_u16();
            let status_text = status.canonical_reason().unwrap_or("OK").to_string();

            let mut resp_headers = Vec::new();
            let mut resp_cookies = Vec::new();
            for (k, v) in resp.headers().iter() {
                let key_str = k.as_str().to_string();
                let val_str = v.to_str().unwrap_or("<binario>").to_string();
                if key_str.eq_ignore_ascii_case("set-cookie") {
                    if let Some(first_part) = val_str.split(';').next() {
                        if let Some((ck, cv)) = first_part.split_once('=') {
                            resp_cookies.push((ck.trim().to_string(), cv.trim().to_string()));
                        }
                    }
                }
                resp_headers.push((key_str, val_str));
            }

            match resp.bytes().await {
                Ok(bytes) => {
                    let size_bytes = bytes.len();
                    let raw_text = String::from_utf8_lossy(&bytes).to_string();

                    let (formatted_body, format) =
                        detect_and_format_body(&raw_text, &resp_headers);
                    let is_json = format == ResponseFormat::Json;

                    let parsed_json: Option<serde_json::Value> =
                        serde_json::from_str(raw_text.trim()).ok();

                    let test_results = evaluate_assertions(
                        &req.assertions,
                        status_code,
                        elapsed_ms,
                        &raw_text,
                        &resp_headers,
                        parsed_json.as_ref(),
                    );

                    let extracted_vars =
                        evaluate_extractions(&req.extractions, parsed_json.as_ref());

                    HttpResponseData {
                        status_code,
                        status_text,
                        elapsed_ms,
                        size_bytes,
                        headers: resp_headers,
                        cookies: resp_cookies,
                        raw_body: raw_text,
                        body: formatted_body,
                        format,
                        is_json,
                        error: None,
                        test_results,
                        extracted_vars,
                    }
                }
                Err(e) => HttpResponseData {
                    status_code,
                    status_text,
                    elapsed_ms,
                    size_bytes: 0,
                    headers: resp_headers,
                    cookies: resp_cookies,
                    raw_body: String::new(),
                    body: String::new(),
                    format: ResponseFormat::Text,
                    is_json: false,
                    error: Some(format!("Error leyendo el cuerpo de la respuesta: {}", e)),
                    test_results: vec![],
                    extracted_vars: vec![],
                },
            }
        }
        Err(e) => HttpResponseData {
            status_code: 0,
            status_text: "Error de Red".to_string(),
            elapsed_ms,
            size_bytes: 0,
            headers: vec![],
            cookies: vec![],
            raw_body: String::new(),
            body: String::new(),
            format: ResponseFormat::Text,
            is_json: false,
            error: Some(format!("Fallo en la petición: {}", e)),
            test_results: vec![],
            extracted_vars: vec![],
        },
    }
}

/// Genera el código fuente listo para copiar en distintos lenguajes para la petición actual
pub fn generate_code_snippet(
    req: &HttpRequestItem,
    env: Option<&Environment>,
    lang: CodeLang,
) -> String {
    let interpolated = interpolate_vars(req.url.trim(), env);
    let url = normalize_url(&interpolated);

    let mut headers: Vec<(String, String)> = req
        .headers
        .iter()
        .filter(|h| h.enabled && !h.key.trim().is_empty())
        .map(|h| {
            (
                interpolate_vars(h.key.trim(), env),
                interpolate_vars(&h.value, env),
            )
        })
        .collect();

    match &req.auth {
        AuthConfig::None => {}
        AuthConfig::Bearer { token } => {
            let t = interpolate_vars(token.trim(), env);
            if !t.is_empty() {
                headers.push(("Authorization".to_string(), format!("Bearer {}", t)));
            }
        }
        AuthConfig::Basic { username, password } => {
            let u = interpolate_vars(username, env);
            let p = interpolate_vars(password, env);
            let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", u, p));
            headers.push(("Authorization".to_string(), format!("Basic {}", encoded)));
        }
        AuthConfig::ApiKey {
            key,
            value,
            in_query: false,
        } => {
            if !key.trim().is_empty() {
                headers.push((
                    interpolate_vars(key.trim(), env),
                    interpolate_vars(value, env),
                ));
            }
        }
        _ => {}
    }

    let body_text = interpolate_vars(&req.body_content, env);

    match lang {
        CodeLang::Curl => {
            let mut lines = vec![format!("curl -X {} \"{}\"", req.method.as_str(), url)];
            for (k, v) in &headers {
                lines.push(format!("  -H \"{}: {}\"", k, v.replace('"', "\\\"")));
            }
            match req.body_type {
                BodyType::None => {}
                BodyType::Json | BodyType::RawText => {
                    if !body_text.trim().is_empty() {
                        lines.push(format!("  -d '{}'", body_text.replace('\'', "'\\''")));
                    }
                }
                BodyType::FormUrlEncoded => {
                    let pairs: Vec<String> = req
                        .form_data
                        .iter()
                        .filter(|f| f.enabled && !f.key.trim().is_empty())
                        .map(|f| format!("{}={}", f.key.trim(), f.value))
                        .collect();
                    if !pairs.is_empty() {
                        lines.push(format!("  --data-urlencode \"{}\"", pairs.join("&")));
                    }
                }
                BodyType::MultipartForm => {
                    for f in &req.multipart_data {
                        if f.enabled && !f.key.trim().is_empty() {
                            match f.kind {
                                MultipartKind::Text => {
                                    lines.push(format!("  -F \"{}={}\"", f.key.trim(), f.value));
                                }
                                MultipartKind::File => {
                                    lines.push(format!("  -F \"{}=@{}\"", f.key.trim(), f.value));
                                }
                            }
                        }
                    }
                }
            }
            lines.join(" \\\n")
        }
        CodeLang::JavaScriptFetch => {
            let mut out = String::new();
            out.push_str(&format!("const response = await fetch(\"{}\", {{\n", url));
            out.push_str(&format!("  method: \"{}\",\n", req.method.as_str()));
            if !headers.is_empty() {
                out.push_str("  headers: {\n");
                for (k, v) in &headers {
                    out.push_str(&format!("    \"{}\": \"{}\",\n", k, v.replace('"', "\\\"")));
                }
                out.push_str("  },\n");
            }
            if matches!(req.body_type, BodyType::Json | BodyType::RawText)
                && !body_text.trim().is_empty()
            {
                out.push_str(&format!("  body: JSON.stringify({}),\n", body_text.trim()));
            }
            out.push_str("});\n\nconst data = await response.json();\nconsole.log(data);");
            out
        }
        CodeLang::PythonRequests => {
            let mut out = String::from("import requests\n\n");
            out.push_str(&format!("url = \"{}\"\n", url));
            out.push_str("headers = {\n");
            for (k, v) in &headers {
                out.push_str(&format!("    \"{}\": \"{}\",\n", k, v.replace('"', "\\\"")));
            }
            out.push_str("}\n");
            if matches!(req.body_type, BodyType::Json | BodyType::RawText)
                && !body_text.trim().is_empty()
            {
                out.push_str(&format!("payload = '''{}'''\n\n", body_text));
                out.push_str(&format!(
                    "response = requests.request(\"{}\", url, headers=headers, data=payload)\n",
                    req.method.as_str()
                ));
            } else {
                out.push_str(&format!(
                    "\nresponse = requests.request(\"{}\", url, headers=headers)\n",
                    req.method.as_str()
                ));
            }
            out.push_str("print(response.status_code)\nprint(response.text)");
            out
        }
        CodeLang::RustReqwest => {
            let mut out = String::from(
                "#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n",
            );
            out.push_str("    let client = reqwest::Client::new();\n");
            out.push_str(&format!(
                "    let response = client\n        .{}(\"{}\")\n",
                req.method.as_str().to_lowercase(),
                url
            ));
            for (k, v) in &headers {
                out.push_str(&format!(
                    "        .header(\"{}\", \"{}\")\n",
                    k,
                    v.replace('"', "\\\"")
                ));
            }
            if matches!(req.body_type, BodyType::Json | BodyType::RawText)
                && !body_text.trim().is_empty()
            {
                out.push_str(&format!("        .body(r#\"{}\"#)\n", body_text));
            }
            out.push_str("        .send()\n        .await?;\n\n");
            out.push_str("    println!(\"Status: {}\", response.status());\n");
            out.push_str("    println!(\"{}\", response.text().await?);\n    Ok(())\n}");
            out
        }
        CodeLang::CSharpHttpClient => {
            let mut out = String::from("using var client = new HttpClient();\n");
            out.push_str(&format!(
                "var request = new HttpRequestMessage(HttpMethod.{}, \"{}\");\n",
                match req.method {
                    HttpMethod::GET => "Get",
                    HttpMethod::POST => "Post",
                    HttpMethod::PUT => "Put",
                    HttpMethod::DELETE => "Delete",
                    HttpMethod::PATCH => "Patch",
                    HttpMethod::HEAD => "Head",
                    HttpMethod::OPTIONS => "Options",
                },
                url
            ));
            for (k, v) in &headers {
                if !k.eq_ignore_ascii_case("content-type") {
                    out.push_str(&format!(
                        "request.Headers.TryAddWithoutValidation(\"{}\", \"{}\");\n",
                        k,
                        v.replace('"', "\\\"")
                    ));
                }
            }
            if matches!(req.body_type, BodyType::Json | BodyType::RawText)
                && !body_text.trim().is_empty()
            {
                out.push_str(&format!(
                    "request.Content = new StringContent(@\"{}\", System.Text.Encoding.UTF8, \"application/json\");\n",
                    body_text.replace('"', "\"\"")
                ));
            }
            out.push_str("\nvar response = await client.SendAsync(request);\n");
            out.push_str("Console.WriteLine(await response.Content.ReadAsStringAsync());");
            out
        }
    }
}

/// Parsea un comando `curl` y devuelve un `HttpRequestItem` configurado
pub fn parse_curl_command(curl_input: &str) -> Result<HttpRequestItem, String> {
    let cleaned = curl_input
        .replace("\\\r\n", " ")
        .replace("\\\n", " ")
        .trim()
        .to_string();

    if cleaned.is_empty() {
        return Err("El comando cURL está vacío.".to_string());
    }

    // Tokenizar respetando comillas simples y dobles
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for ch in cleaned.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' && !in_single {
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double {
            in_single = !in_single;
            continue;
        }
        if ch == '"' && !in_single {
            in_double = !in_double;
            continue;
        }
        if ch.is_whitespace() && !in_single && !in_double {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    let mut req = HttpRequestItem {
        name: "Importado desde cURL".to_string(),
        headers: Vec::new(),
        query_params: Vec::new(),
        body_type: BodyType::None,
        body_content: String::new(),
        ..Default::default()
    };

    let mut method_explicit = false;
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        match t.as_str() {
            "curl" => {}
            "-X" | "--request" => {
                if i + 1 < tokens.len() {
                    req.method = HttpMethod::from_str_loose(&tokens[i + 1]);
                    method_explicit = true;
                    i += 1;
                }
            }
            "-H" | "--header" => {
                if i + 1 < tokens.len() {
                    if let Some((k, v)) = tokens[i + 1].split_once(':') {
                        req.headers.push(KeyValue::new(k.trim(), v.trim()));
                    }
                    i += 1;
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" => {
                if i + 1 < tokens.len() {
                    let data = tokens[i + 1].clone();
                    req.body_content = data.clone();
                    if data.trim().starts_with('{') || data.trim().starts_with('[') {
                        req.body_type = BodyType::Json;
                    } else {
                        req.body_type = BodyType::RawText;
                    }
                    if !method_explicit {
                        req.method = HttpMethod::POST;
                    }
                    i += 1;
                }
            }
            "-u" | "--user" => {
                if i + 1 < tokens.len() {
                    let (u, p) = tokens[i + 1]
                        .split_once(':')
                        .unwrap_or((&tokens[i + 1], ""));
                    req.auth = AuthConfig::Basic {
                        username: u.to_string(),
                        password: p.to_string(),
                    };
                    i += 1;
                }
            }
            other => {
                if !other.starts_with('-')
                    && (other.contains('.')
                        || other.contains("localhost")
                        || other.starts_with("http"))
                {
                    req.url = other.to_string();
                }
            }
        }
        i += 1;
    }

    if req.url.is_empty() {
        return Err("No se encontró ninguna URL en el comando cURL.".to_string());
    }

    // Extraer query params de la URL importada
    if let Some((_, query)) = req.url.split_once('?') {
        for pair in query.split('&') {
            if !pair.is_empty() {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                req.query_params.push(KeyValue::new(k, v));
            }
        }
    }
    if req.query_params.is_empty() {
        req.query_params.push(KeyValue::empty());
    }
    if req.headers.is_empty() {
        req.headers.push(KeyValue::empty());
    }

    Ok(req)
}

/// Importa colecciones desde un archivo JSON (soporta formato nativo Rustman y Postman Collection v2.1)
pub fn import_collections_from_json(json_str: &str) -> Result<Vec<Collection>, String> {
    // 1. Intentar como Vec<Collection> de Rustman
    if let Ok(cols) = serde_json::from_str::<Vec<Collection>>(json_str) {
        return Ok(cols);
    }
    // 2. Intentar como una sola Collection de Rustman
    if let Ok(col) = serde_json::from_str::<Collection>(json_str) {
        return Ok(vec![col]);
    }
    // 3. Intentar como Postman Collection v2.1
    let val: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON inválido: {}", e))?;

    if let Some(items) = val.get("item").and_then(|i| i.as_array()) {
        let col_name = val
            .get("info")
            .and_then(|info| info.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("Colección Postman Importada")
            .to_string();

        let mut requests = Vec::new();
        parse_postman_items(items, &mut requests);

        return Ok(vec![Collection {
            id: Uuid::new_v4(),
            name: col_name,
            requests,
        }]);
    }

    Err("El archivo JSON no coincide con el formato de Rustman ni con Postman v2.1.".to_string())
}

fn parse_postman_items(items: &[serde_json::Value], out: &mut Vec<HttpRequestItem>) {
    for item in items {
        // Si es una subcarpeta de Postman con más items
        if let Some(sub_items) = item.get("item").and_then(|i| i.as_array()) {
            parse_postman_items(sub_items, out);
            continue;
        }

        let name = item
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("Petición Postman")
            .to_string();

        if let Some(req_obj) = item.get("request") {
            let method_str = req_obj
                .get("method")
                .and_then(|m| m.as_str())
                .unwrap_or("GET");
            let method = HttpMethod::from_str_loose(method_str);

            let url = match req_obj.get("url") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(serde_json::Value::Object(u_obj)) => u_obj
                    .get("raw")
                    .and_then(|r| r.as_str())
                    .unwrap_or("")
                    .to_string(),
                _ => String::new(),
            };

            let mut headers = Vec::new();
            if let Some(h_arr) = req_obj.get("header").and_then(|h| h.as_array()) {
                for h in h_arr {
                    let k = h.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    let v = h.get("value").and_then(|v| v.as_str()).unwrap_or("");
                    if !k.is_empty() {
                        headers.push(KeyValue::new(k, v));
                    }
                }
            }
            if headers.is_empty() {
                headers.push(KeyValue::empty());
            }

            let mut body_type = BodyType::None;
            let mut body_content = String::new();
            if let Some(body_obj) = req_obj.get("body") {
                if let Some(raw) = body_obj.get("raw").and_then(|r| r.as_str()) {
                    body_content = raw.to_string();
                    if raw.trim().starts_with('{') || raw.trim().starts_with('[') {
                        body_type = BodyType::Json;
                    } else {
                        body_type = BodyType::RawText;
                    }
                }
            }

            let mut query_params = Vec::new();
            if let Some((_, query)) = url.split_once('?') {
                for pair in query.split('&') {
                    if !pair.is_empty() {
                        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
                        query_params.push(KeyValue::new(k, v));
                    }
                }
            }
            if query_params.is_empty() {
                query_params.push(KeyValue::empty());
            }

            out.push(HttpRequestItem {
                id: Uuid::new_v4(),
                name,
                method,
                url,
                query_params,
                headers,
                body_type,
                body_content,
                ..Default::default()
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_osgroyco_url() {
        let mut req = HttpRequestItem::default();
        req.method = HttpMethod::POST;
        req.url = "https://osgroyco.mx/sistemadeincidencias-uat/api/v2/auth/login".to_string();
        req.body_type = BodyType::Json;
        req.body_content = "{}".to_string();

        let resp = send_http_request(req, None, vec![]).await;
        println!("STATUS: {}, ERROR: {:?}, BODY: {}", resp.status_code, resp.error, resp.body);
        assert!(resp.error.is_none(), "Expected no error, got: {:?}", resp.error);
        assert_eq!(resp.status_code, 401);
    }
}
