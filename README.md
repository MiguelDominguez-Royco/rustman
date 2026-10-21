# 🚀 Rustman — Cliente HTTP de Escritorio en Rust

**Rustman** es una aplicación nativa de escritorio al estilo **Postman**, construida en **Rust** utilizando [`egui`](https://github.com/emilk/egui) / `eframe` para la interfaz gráfica y `reqwest` + `tokio` para peticiones asíncronas ultrarrápidas.

---

## ✨ Características incluidas

* **Métodos HTTP con código de color:** `GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`.
* **Constructor completo de peticiones:**
  * Editor de **Query Params** con casillas de activación/desactivación.
  * Editor de **Headers** personalizados.
  * **Autenticación integrada:** `Bearer Token`, `Basic Auth` (codificación Base64 automática) y `API Key` (en Header o Query Param).
  * **Body versátil:** `JSON` (con botón para formatear/embellecer JSON), `Texto Plano` y `x-www-form-urlencoded`.
* **Variables de Entorno (`{{variable}}`):**
  * Crea múltiples entornos (ej. *Desarrollo*, *Producción*) y usa variables como `{{base_url}}` o `{{api_token}}` en la URL, Headers, Params, Auth o Body.
* **Colecciones e Historial persistente:**
  * Guarda tus peticiones en colecciones organizadas y consulta el historial automático con código de estado y hora.
  * Los datos se guardan automáticamente en disco (`%LOCALAPPDATA%\rustman\rustman_data.json`).
* **Visor de Respuestas:**
  * Muestra código HTTP con colores semánticos, tiempo de respuesta en milisegundos (`ms`), tamaño en `bytes`, cabeceras de respuesta y cuerpo JSON auto-formateado con botón de copiado rápido.
* **Atajo de teclado:** Presiona `Ctrl + Enter` en cualquier momento para disparar la petición.

---

## 🛠️ Requisitos e Instalación de Rust en Windows

Si aún no tienes **Rust** instalado en tu equipo:

1. Abre **PowerShell** e instala `rustup` con `winget`:
   ```powershell
   winget install Rustlang.Rustup
   ```
   *(O descárgalo directamente desde [https://rustup.rs](https://rustup.rs)).*
2. Reinicia tu terminal para que `cargo` y `rustc` estén disponibles en tu `PATH`.

---

## ▶️ Cómo compilar y ejecutar

Abre una terminal en la carpeta del proyecto (`C:\Users\migdom\Documents\rustman`) y ejecuta:

```powershell
# Ejecutar en modo desarrollo
cargo run

# O compilar el ejecutable optimizado (.exe) para producción
cargo build --release
```

El archivo ejecutable independiente quedará listo en:
`target\release\rustman.exe`
