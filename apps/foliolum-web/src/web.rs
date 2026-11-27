//! The browser shell: web `Services`, drag-and-drop, and the eframe web runner.
//!
//! From DesignCraft's `apps/designcraft-web/src/web.rs` (MIT OR Apache-2.0) at the pinned commit, with
//! Foliolum's names and element ids, .pub in File ▸ Open, and Publisher files opened by `pub_open`.

use super::browser_policy::{has_query_flag, mime_for};
use super::prefs_store::{self, PrefsSync};
use crate::graphics::{GraphicsState, RecoveryDocument, recovery_documents};
use designcraft_engine::Session;
use designcraft_ui_egui::{DesignApp, ImportRequest, ImportedFile, Inbox, Services};
use wasm_bindgen::JsCast as _;

const DOC_EXTS: &[&str] = &["pub", "designcraft", "idml", "indd", "indt"];
const IMAGE_EXTS: &[&str] = &[
    "png",
    "jpg",
    "jpeg",
    "gif",
    "webp",
    "tif",
    "tiff",
    "bmp",
    "psd",
    "svg",
    "pdf",
    "ai",
    "txt",
    "docx",
    "rtf",
    "md",
    "xlsx",
    "idml",
    "designcraft",
];
/// Graphics a relink reads (the image and vector formats placing accepts).
const GRAPHIC_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "tif", "tiff", "bmp", "psd", "svg", "pdf", "ai"];
const CANVAS_ID: &str = "foliolum_canvas";
const LOADING_ID: &str = "foliolum_loading";

pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    designcraft_ui_egui::brand::set(crate::BRAND);
    designcraft_ui_egui::shell::set(foliolum_shell::SHELL);
    foliolum_shell::set_app_actions(app_action);
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            log::error!("no document");
            return;
        };
        if let Err(e) = create_error_panel(&document) {
            log::error!("couldn't create graphics error panel: {e:?}");
            if let Some(el) = document.get_element_by_id(LOADING_ID) {
                el.set_text_content(Some("Foliolum couldn't start. Reload this page to try again."));
            }
            return;
        }
        let graphics = GraphicsState::default();
        let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            graphics_failed(&graphics, "Foliolum failed to start", "Reload this page to try again.", &format!("Missing canvas {CANVAS_ID}."));
            return;
        };
        let mut options = eframe::WebOptions::default();
        if has_query_flag(&query(), "webgl")
            && let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup
        {
            create.instance_descriptor.backends = eframe::wgpu::Backends::GL;
        }
        let runner = eframe::WebRunner::new();
        let creator_graphics = graphics.clone();
        let result = runner
            .start(
                canvas.clone(),
                options,
                Box::new(move |cc| {
                    if let Some(rs) = &cc.wgpu_render_state {
                        log::info!("foliolum-web: wgpu backend {:?}", rs.adapter.get_info().backend);
                        let device_graphics = creator_graphics.clone();
                        // This also observes an already-lost WebGPU device through its lost promise.
                        rs.device.set_device_lost_callback(move |reason, message| {
                            graphics_failed(&device_graphics, GRAPHICS_TITLE, GRAPHICS_MESSAGE, &format!("Device loss ({reason:?}): {message}"));
                        });
                    }
                    crate::fonts_web::start(cc.egui_ctx.clone());
                    let inbox: Inbox = Inbox::default();
                    let mut app = DesignApp::new(crate::new_session(), services(inbox.clone(), cc.egui_ctx.clone()));
                    if has_query_flag(&query(), "sample") {
                        let _ = app.run("file.newSample", serde_json::json!({}));
                    }
                    if let Some(name) = crate::browser_policy::demo_name(&query()) {
                        open_demo(name, app.import_request("open"), inbox.clone(), cc.egui_ctx.clone());
                    }
                    // The preferences saved by an earlier visit (as the desktop app reads prefs.json).
                    let stored = local_storage().and_then(|s| s.get_item(prefs_store::KEY).ok().flatten());
                    if let Some(prefs) = prefs_store::decode(stored.as_deref()) {
                        app.session.prefs = prefs;
                    }
                    let prefs_sync = PrefsSync::new(stored);
                    Ok(Box::new(WebShell {
                        prefs_sync,
                        app,
                        inbox,
                        read_errors: Default::default(),
                        graphics: creator_graphics,
                        held: Vec::new(),
                        sign: Default::default(),
                        smart: Default::default(),
                        profile: Default::default(),
                    }))
                }),
            )
            .await;
        match result {
            Ok(()) => {
                if let Err(e) = install_recovery_controls(&runner, &canvas, &graphics) {
                    graphics_failed(&graphics, GRAPHICS_TITLE, GRAPHICS_MESSAGE, &format!("Couldn't install graphics recovery controls: {e:?}"));
                    if let Some(shell) = runner.app_mut::<WebShell>()
                        && let Err(e) = prepare_document_recovery(&runner, &shell.app.session)
                    {
                        recovery_controls_failed(&e);
                    }
                }
                // A loss callback can run during startup. Never hide its failure panel.
                if !graphics.has_failed()
                    && let Some(el) = document.get_element_by_id(LOADING_ID)
                {
                    el.remove();
                }
            }
            Err(e) => {
                graphics_failed(
                    &graphics,
                    "Foliolum failed to start",
                    "Reload this page to try again. A browser with WebGPU or WebGL2 is required.",
                    &format!("{e:?}"),
                );
                if let Some(shell) = runner.app_mut::<WebShell>()
                    && let Err(e) = prepare_document_recovery(&runner, &shell.app.session)
                {
                    recovery_controls_failed(&e);
                }
            }
        }
    });
}

const ERROR_ID: &str = "foliolum_graphics_error";
const ERROR_TITLE_ID: &str = "foliolum_graphics_title";
const ERROR_MESSAGE_ID: &str = "foliolum_graphics_message";
const ERROR_DETAIL_ID: &str = "foliolum_graphics_detail";
const ERROR_ACTIONS_ID: &str = "foliolum_graphics_actions";
const ERROR_STATUS_ID: &str = "foliolum_graphics_status";
const ERROR_DOCUMENTS_ID: &str = "foliolum_graphics_documents";
const GRAPHICS_LOST_EVENT: &str = "foliolumgraphicslost";
const GRAPHICS_TITLE: &str = "Graphics connection lost";
const GRAPHICS_MESSAGE: &str = "Editing has stopped. Save a copy of each open document, then reload this page and reopen the copies. Reloading discards changes you have not downloaded.";

fn create_error_panel(document: &web_sys::Document) -> Result<(), wasm_bindgen::JsValue> {
    let panel: web_sys::HtmlElement = document.create_element("div")?.dyn_into()?;
    panel.set_id(ERROR_ID);
    panel.set_tab_index(-1);
    panel.set_attribute("role", "alertdialog")?;
    panel.set_attribute("aria-modal", "true")?;
    panel.set_attribute("aria-labelledby", ERROR_TITLE_ID)?;
    panel.set_attribute("aria-describedby", ERROR_MESSAGE_ID)?;
    // DOM content remains visible when the GPU can no longer paint the egui canvas.
    let colors = designcraft_ui_egui::theme::Tokens::for_brightness(designcraft_ui_egui::theme::Brightness::Dark);
    panel.style().set_css_text(&format!(
        "position:fixed;inset:0;z-index:1;display:none;flex-direction:column;align-items:center;justify-content:safe center;box-sizing:border-box;padding:24px;overflow:auto;background:{};color:{};font:14px system-ui,sans-serif;gap:12px;",
        css_color(colors.app_bar), css_color(colors.text),
    ));
    for (tag, id) in [("h1", ERROR_TITLE_ID), ("p", ERROR_MESSAGE_ID), ("div", ERROR_ACTIONS_ID), ("p", ERROR_STATUS_ID), ("pre", ERROR_DETAIL_ID)] {
        let child: web_sys::HtmlElement = document.create_element(tag)?.dyn_into()?;
        child.set_id(id);
        child.style().set_css_text("margin:0;max-width:640px;white-space:pre-wrap;overflow-wrap:anywhere;");
        if id == ERROR_ACTIONS_ID {
            child.style().set_css_text("display:flex;flex-direction:column;gap:8px;max-width:100%;");
            add_reload_control(document, &child)?;
        } else if id == ERROR_STATUS_ID {
            child.set_attribute("aria-live", "polite")?;
        }
        panel.append_child(&child)?;
    }
    document.body().ok_or_else(|| wasm_bindgen::JsValue::from_str("no body"))?.append_child(&panel)?;
    Ok(())
}

fn graphics_failed(graphics: &GraphicsState, title: &str, message: &str, detail: &str) {
    if !graphics.fail() {
        return;
    }
    log::error!("foliolum-web: {detail}");
    if let Err(e) = show_error(title, message, detail) {
        log::error!("couldn't show graphics error: {e:?}");
    }
    // A device callback may run while AppRunner is borrowed. Defer its notification;
    // the WebGL listener already holding AppRunner captures documents directly.
    if let Some(window) = web_sys::window()
        && let Some(canvas) = window.document().and_then(|doc| doc.get_element_by_id(CANVAS_ID))
    {
        let notify = wasm_bindgen::closure::Closure::once_into_js(move || {
            if let Err(e) = web_sys::Event::new(GRAPHICS_LOST_EVENT).and_then(|event| canvas.dispatch_event(&event)) {
                recovery_controls_failed(&e);
            }
        });
        window.queue_microtask(notify.unchecked_ref());
    }
}

fn show_error(title: &str, message: &str, detail: &str) -> Result<(), wasm_bindgen::JsValue> {
    let document = web_sys::window().and_then(|w| w.document()).ok_or_else(|| wasm_bindgen::JsValue::from_str("no document"))?;
    let panel: web_sys::HtmlElement =
        document.get_element_by_id(ERROR_ID).ok_or_else(|| wasm_bindgen::JsValue::from_str("no graphics error panel"))?.dyn_into()?;
    for (id, text) in [(ERROR_TITLE_ID, title), (ERROR_MESSAGE_ID, message), (ERROR_DETAIL_ID, detail)] {
        if let Some(el) = document.get_element_by_id(id) {
            el.set_text_content(Some(text));
        }
    }
    panel.style().set_property("display", "flex")?;
    if let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok()) {
        canvas.style().set_property("visibility", "hidden")?;
    }
    if let Some(loading) = document.get_element_by_id(LOADING_ID).and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok()) {
        loading.style().set_property("display", "none")?;
    }
    // Include eframe's hidden text input: neither pointer nor keyboard focus may reach the editor.
    let mut sibling = document.body().and_then(|body| body.first_element_child());
    while let Some(el) = sibling {
        sibling = el.next_element_sibling();
        if el.id() != ERROR_ID {
            if let Some(html) = el.dyn_ref::<web_sys::HtmlElement>() {
                html.set_inert(true);
            }
            el.set_attribute("aria-hidden", "true")?;
        }
    }
    panel.focus()?;
    Ok(())
}

/// Recovery listeners are owned by their DOM targets, independently of WebRunner.
fn add_dom_event_listener(
    target: &web_sys::EventTarget,
    event_name: &str,
    callback: impl FnMut(web_sys::Event) + 'static,
) -> Result<(), wasm_bindgen::JsValue> {
    let callback = wasm_bindgen::closure::Closure::wrap(Box::new(callback) as Box<dyn FnMut(web_sys::Event)>);
    target.add_event_listener_with_callback(event_name, callback.as_ref().unchecked_ref())?;
    // EventTarget retains the JS function; weak references reclaim its captures with the target.
    let _ = callback.into_js_value();
    Ok(())
}

fn css_color(color: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

fn recovery_button(document: &web_sys::Document) -> Result<web_sys::HtmlElement, wasm_bindgen::JsValue> {
    let button: web_sys::HtmlElement = document.create_element("button")?.dyn_into()?;
    button.set_attribute("type", "button")?;
    let colors = designcraft_ui_egui::theme::Tokens::for_brightness(designcraft_ui_egui::theme::Brightness::Dark);
    button.style().set_css_text(&format!(
        "box-sizing:border-box;max-width:100%;padding:6px 12px;border:1px solid {};border-radius:{}px;background:{};color:{};font:inherit;white-space:normal;overflow-wrap:anywhere;cursor:pointer;",
        css_color(colors.field_border), colors.radius, css_color(colors.button), css_color(colors.text),
    ));
    Ok(button)
}

fn add_reload_control(document: &web_sys::Document, actions: &web_sys::HtmlElement) -> Result<(), wasm_bindgen::JsValue> {
    let reload = recovery_button(document)?;
    reload.set_text_content(Some("Reload"));
    add_dom_event_listener(&reload, "click", move |event| {
        event.stop_propagation();
        if let Some(window) = web_sys::window()
            && let Err(e) = window.location().reload()
        {
            log::error!("couldn't reload: {e:?}");
        }
    })?;
    actions.append_child(&reload)?;
    Ok(())
}

fn install_recovery_controls(
    runner: &eframe::WebRunner,
    canvas: &web_sys::HtmlCanvasElement,
    graphics: &GraphicsState,
) -> Result<(), wasm_bindgen::JsValue> {
    let loss_runner = runner.clone();
    runner.add_event_listener(canvas, GRAPHICS_LOST_EVENT, move |_: web_sys::Event, app_runner| {
        let shell = app_runner.app_mut::<WebShell>();
        if shell.graphics.has_failed()
            && let Err(e) = prepare_document_recovery(&loss_runner, &shell.app.session)
        {
            recovery_controls_failed(&e);
        }
    })?;
    let context_graphics = graphics.clone();
    let context_runner = runner.clone();
    // wgpu's GLES backend does not observe browser WebGL context-loss events.
    runner.add_event_listener(canvas, "webglcontextlost", move |_: web_sys::Event, app_runner| {
        graphics_failed(&context_graphics, GRAPHICS_TITLE, GRAPHICS_MESSAGE, "The browser lost the WebGL graphics context.");
        if let Err(e) = prepare_document_recovery(&context_runner, &app_runner.app_mut::<WebShell>().app.session) {
            recovery_controls_failed(&e);
        }
    })?;

    // A WebGPU device can be lost while start() is still awaiting initialization.
    if graphics.has_failed()
        && let Some(shell) = runner.app_mut::<WebShell>()
    {
        prepare_document_recovery(runner, &shell.app.session)?;
    }
    Ok(())
}

fn prepare_document_recovery(runner: &eframe::WebRunner, session: &Session) -> Result<(), wasm_bindgen::JsValue> {
    let window = web_sys::window().ok_or_else(|| wasm_bindgen::JsValue::from_str("no window"))?;
    let documents = recovery_documents(session);
    if add_document_recovery_controls(documents)? {
        // Controls now own the snapshots. Release AppRunner's borrow before stopping painting
        // and removing its listeners/ResizeObserver; never discard documents on setup failure.
        let runner = runner.clone();
        let stop = wasm_bindgen::closure::Closure::once_into_js(move || runner.destroy());
        window.queue_microtask(stop.unchecked_ref());
    }
    Ok(())
}

fn recovery_controls_failed(error: &wasm_bindgen::JsValue) {
    log::error!("couldn't prepare recovery downloads: {error:?}");
    if let Some(status) = web_sys::window().and_then(|w| w.document()).and_then(|doc| doc.get_element_by_id(ERROR_STATUS_ID)) {
        status.set_text_content(Some("Couldn't prepare document downloads. Keep this page open to preserve unsaved documents."));
    }
}

fn add_document_recovery_controls(snapshots: Vec<RecoveryDocument>) -> Result<bool, wasm_bindgen::JsValue> {
    let document = web_sys::window().and_then(|w| w.document()).ok_or_else(|| wasm_bindgen::JsValue::from_str("no document"))?;
    let actions = document.get_element_by_id(ERROR_ACTIONS_ID).ok_or_else(|| wasm_bindgen::JsValue::from_str("no recovery actions"))?;
    if document.get_element_by_id(ERROR_DOCUMENTS_ID).is_some() {
        return Ok(false);
    }
    let documents: web_sys::HtmlElement = document.create_element("div")?.dyn_into()?;
    documents.set_id(ERROR_DOCUMENTS_ID);
    documents.style().set_css_text("display:flex;flex-direction:column;gap:8px;max-width:100%;");
    for snapshot in snapshots {
        let button = recovery_button(&document)?;
        button.set_text_content(Some(&format!("Save a Copy: {}", snapshot.title())));
        add_dom_event_listener(&button, "click", move |event| {
            event.stop_propagation();
            let result = snapshot.save_copy().and_then(|(name, bytes)| {
                download(&name, &bytes)?;
                Ok(format!("Requested download of {name}. Check your browser downloads before reloading."))
            });
            let text = result.unwrap_or_else(|e| e);
            if let Some(status) = web_sys::window().and_then(|w| w.document()).and_then(|doc| doc.get_element_by_id(ERROR_STATUS_ID)) {
                status.set_text_content(Some(&text));
            }
        })?;
        documents.append_child(&button)?;
    }
    // Build off-DOM so a setup error leaves the session intact and permits a clean retry.
    // The inserted element's id also prevents duplicate buttons on repeated loss notifications.
    actions.insert_before(&documents, actions.first_child().as_ref())?;
    Ok(true)
}

/// Opens the invented demo publication `demo/<name>.pub` (`?demo=<name>`), for the guides' screenshots
/// and videos: fetched from this site and handed in as if opened from disk.
fn open_demo(name: &'static str, request: ImportRequest, inbox: Inbox, ctx: egui::Context) {
    wasm_bindgen_futures::spawn_local(async move {
        let url = format!("demo/{name}.pub");
        match fetch_bytes(&url).await {
            Ok(bytes) => {
                inbox.lock().unwrap_or_else(|e| e.into_inner()).push(ImportedFile { request, name: format!("{name}.pub"), bytes });
                ctx.request_repaint();
            }
            Err(e) => log::error!("{url}: {e}"),
        }
    });
}

async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response = wasm_bindgen_futures::JsFuture::from(window.fetch_with_str(url))
        .await
        .map_err(|e| format!("{e:?}"))?
        .dyn_into()
        .map_err(|e| format!("{e:?}"))?;
    if !response.ok() {
        return Err(format!("HTTP {}", response.status()));
    }
    let buffer = wasm_bindgen_futures::JsFuture::from(response.array_buffer().map_err(|e| format!("{e:?}"))?).await.map_err(|e| format!("{e:?}"))?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

/// The page's local storage; `None` where the browser refuses it (some private modes).
/// The Our details saved in this browser (empty when there are none).
pub fn stored_profile() -> crate::profile::Profile {
    let stored = local_storage().and_then(|s| s.get_item(crate::profile::STORAGE_KEY).ok().flatten());
    crate::profile::Profile::decode(stored.as_deref())
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn query() -> String {
    web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
}

/// Wraps the app to read dropped files asynchronously (browsers can't read them synchronously)
/// and feed them through the inbox.
struct WebShell {
    app: DesignApp,
    inbox: Inbox,
    /// Why a dropped file couldn't be read, for the status bar (the read finishes outside a frame).
    read_errors: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    graphics: GraphicsState,
    /// Files that arrived before the fonts did ([`crate::fonts_web`]), in order.
    held: Vec<ImportedFile>,
    /// Send for signature ([`crate::sign`]).
    sign: crate::sign::Dialog,
    /// Smart frames ([`crate::smart`]).
    smart: crate::smart::Dialog,
    /// Our details ([`crate::profile`]).
    profile: crate::profile::Dialog,
    /// Writes the preferences to local storage when they change.
    prefs_sync: PrefsSync,
}

/// The ribbon's buttons for things outside DesignCraft ([`foliolum_shell::set_app_actions`]).
fn app_action(app: &mut DesignApp, id: &str) {
    match id {
        "sign" => SIGN_REQUESTED.store(true, std::sync::atomic::Ordering::Relaxed),
        "smart" => SMART_REQUESTED.store(true, std::sync::atomic::Ordering::Relaxed),
        "profile" => PROFILE_REQUESTED.store(true, std::sync::atomic::Ordering::Relaxed),
        "copyfit" => {
            let message = match crate::smart::selected_frame(&app.session) {
                Some(id) => crate::copyfit::fit(&mut app.session, id).map(crate::copyfit::Outcome::message).unwrap_or_else(|e| e),
                None => "Select the text box whose text overflows, then choose Make it fit.".to_string(),
            };
            app.status(message);
        }
        _ => {}
    }
}

/// Our details was pressed; the shell opens its dialog on the next frame.
static PROFILE_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Smart frame was pressed; the shell opens its dialog on the next frame.
static SMART_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Send for signature was pressed; the shell opens its dialog on the next frame.
static SIGN_REQUESTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl WebShell {
    fn profile_dialog(&mut self, ctx: &egui::Context) {
        if PROFILE_REQUESTED.swap(false, std::sync::atomic::Ordering::Relaxed) {
            self.profile.profile = stored_profile();
            self.profile.set_message(String::new());
            self.profile.open = true;
        }
        let (p, insert) = match self.profile.show(ctx) {
            Some(crate::profile::Want::Save(p)) => (p, None),
            Some(crate::profile::Want::Insert(p, i)) => (p, Some(i)),
            None => return,
        };
        let kept =
            serde_json::to_string(&p).ok().zip(local_storage()).is_some_and(|(json, s)| s.set_item(crate::profile::STORAGE_KEY, &json).is_ok());
        let result = match insert {
            Some(i) => {
                crate::profile::insert(&mut self.app.session, &p, i).map(|()| format!("Inserted {}.", crate::profile::FIELDS[i].0.to_lowercase()))
            }
            None => crate::profile::apply(&mut self.app.session, &p).map(|n| {
                if n == 0 { "Saved.".to_string() } else { format!("Saved, and updated {n} field{} in this document.", if n == 1 { "" } else { "s" }) }
            }),
        };
        let message = match result {
            Ok(m) if kept => m,
            Ok(m) => format!("{m} This browser couldn't keep the details for next time."),
            Err(e) => e,
        };
        self.profile.set_message(message);
    }

    fn smart_dialog(&mut self, ctx: &egui::Context) {
        if SMART_REQUESTED.swap(false, std::sync::atomic::Ordering::Relaxed) {
            self.smart.open_for(&self.app.session);
        }
        match self.smart.show(ctx) {
            Some(crate::smart::Want::Fill(id, binding)) => {
                let status = match crate::smart::fill(&mut self.app.session, id, &binding) {
                    Ok(()) => format!("Smart frame filled: {} rows", binding.data_rows().len()),
                    Err(e) => e,
                };
                self.app.status(status);
            }
            Some(crate::smart::Want::PickFile) => {
                let slot = self.smart.loaded_slot();
                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let picked = rfd::AsyncFileDialog::new().add_filter("Spreadsheet (CSV, Excel)", &["csv", "tsv", "txt", "xlsx"]).pick_file().await;
                    if let Some(file) = picked {
                        let bytes = file.read().await;
                        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some((file.file_name(), bytes));
                        ctx.request_repaint();
                    }
                });
            }
            None => {}
        }
    }

    fn sign_dialog(&mut self, ctx: &egui::Context) {
        if SIGN_REQUESTED.swap(false, std::sync::atomic::Ordering::Relaxed) {
            let title = self.app.session.active().map(|d| d.title()).unwrap_or_default();
            self.sign.open(&title);
        }
        if !self.sign.open {
            return;
        }
        let marks = self.app.session.doc().map(|d| crate::sign::document_marks(&d.doc)).unwrap_or_default();
        let Some(mut request) = self.sign.show(ctx, &marks) else { return };
        let slot = self.sign.result_slot();
        let pdf = self.app.session.execute("file.exportPdf", &serde_json::json!({})).map_err(|e| e.to_string()).and_then(|r| {
            r["base64"].as_str().map(designcraft_engine::cmd::base64_decode).ok_or_else(|| "the PDF export returned nothing".to_string())
        });
        let ctx = ctx.clone();
        match pdf {
            Ok(pdf) => {
                request.pdf = pdf;
                wasm_bindgen_futures::spawn_local(async move {
                    let outcome = crate::sign_web::run(request).await;
                    *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(outcome);
                    ctx.request_repaint();
                });
            }
            Err(e) => {
                *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(crate::sign::Outcome::Failed(format!("Couldn't make the PDF: {e}")));
                ctx.request_repaint();
            }
        }
    }
}

impl eframe::App for WebShell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let graphics = self.graphics.clone();
        // Changed preferences go to local storage, so they survive a reload.
        let now = ctx.input(|i| i.time);
        if let Some(wait) = self.prefs_sync.wait(now) {
            // Not due yet: a frame then checks, even if nothing else happens until then.
            if let Ok(d) = std::time::Duration::try_from_secs_f64(wait) {
                ctx.request_repaint_after(d);
            }
        } else if self.prefs_sync.due(now)
            && let Some(json) = self.prefs_sync.changed(&self.app.session.prefs)
        {
            // Counted as stored only once written; otherwise it's tried again at the next check.
            let written = match local_storage() {
                Some(storage) => match storage.set_item(prefs_store::KEY, &json) {
                    Ok(()) => true,
                    Err(e) => {
                        log::warn!("couldn't keep the preferences in local storage: {e:?}");
                        false
                    }
                },
                None => false,
            };
            if written {
                self.prefs_sync.stored(json);
            } else if let Some(d) = self.prefs_sync.wait(now).and_then(|w| std::time::Duration::try_from_secs_f64(w).ok()) {
                // Still pending: a frame at the next check retries, even if the app is idle.
                ctx.request_repaint_after(d);
            }
        }
        let errors = std::mem::take(&mut *self.read_errors.lock().unwrap_or_else(|e| e.into_inner()));
        if let Some(error) = errors.into_iter().last() {
            self.app.status(error);
        }
        // Nothing opens until the fonts are in, or it would be laid out with fallback fonts.
        let arrived = std::mem::take(&mut *self.inbox.lock().unwrap_or_else(|e| e.into_inner()));
        if !arrived.is_empty() && self.held.is_empty() && !crate::fonts_web::settled() {
            self.app.status("Loading fonts…".to_string());
        }
        self.held.extend(arrived);
        // Publisher files are opened here; DesignCraft's open routing gets everything else.
        if !self.held.is_empty() && crate::fonts_web::settled() {
            let mut statuses = Vec::new();
            let rest = crate::pub_open::take_pubs(&mut self.app.session, std::mem::take(&mut self.held), |s| statuses.push(s));
            self.inbox.lock().unwrap_or_else(|e| e.into_inner()).extend(rest);
            let failed = crate::fonts_web::failed();
            if failed > 0 {
                statuses.push(format!("{failed} font{} couldn't be loaded; reload the page to try again", if failed == 1 { "" } else { "s" }));
            }
            if let Some(status) = statuses.pop() {
                self.app.status(status);
            }
        }
        let _ = graphics.run_if_healthy(|| self.app.logic(ctx));
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        let _ = self.graphics.run_if_healthy(|| {
            // Dropped files belong to this host iteration's raw input, not to stale logic input:
            // read them here once, before the batch is saved as pending for a hidden pass.
            // Files dropped together arrive together, in their order, so several images load the
            // place cursor as one batch.
            let dropped = std::mem::take(&mut raw.dropped_files);
            if !dropped.is_empty() {
                let request = self.app.import_request("drop");
                let inbox = self.inbox.clone();
                let read_errors = self.read_errors.clone();
                let ctx = ctx.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let files = crate::browser_policy::read_batch(dropped, |f| {
                        let request = request.clone();
                        async move {
                            let name = f.path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dropped".into());
                            let bytes = f.bytes_async().await.map_err(|e| format!("couldn't read dropped file {name}: {e}"))?;
                            Ok::<_, String>(ImportedFile { request, name, bytes })
                        }
                    })
                    .await;
                    match files {
                        Ok(files) => {
                            inbox.lock().unwrap_or_else(|e| e.into_inner()).extend(files);
                            ctx.request_repaint();
                        }
                        Err(error) => {
                            log::error!("{error}");
                            read_errors.lock().unwrap_or_else(|e| e.into_inner()).push(error);
                            ctx.request_repaint();
                        }
                    }
                });
            }
            designcraft_ui_egui::drop_key_name_text(raw, ctx.text_edit_focused() || self.app.canvas_ime());
            self.app.raw_input_hook(ctx, raw);
        });
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let _ = self.graphics.run_if_healthy(|| self.app.ui(ui));
        let ctx = ui.ctx().clone();
        let graphics = self.graphics.clone();
        let _ = graphics.run_if_healthy(|| self.sign_dialog(&ctx));
        let _ = graphics.run_if_healthy(|| self.smart_dialog(&ctx));
        let _ = graphics.run_if_healthy(|| self.profile_dialog(&ctx));
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

fn services(inbox: Inbox, ctx: egui::Context) -> Services {
    let open_inbox = inbox.clone();
    Services {
        open_async: Some(Box::new(move |request: ImportRequest| {
            let inbox = open_inbox.clone();
            let ctx = ctx.clone();
            let dialog = if request.purpose == "swatches" {
                rfd::AsyncFileDialog::new().add_filter("Swatch Exchange (ASE)", &["ase"])
            } else if request.purpose == "shortcuts" {
                rfd::AsyncFileDialog::new().add_filter("Shortcut set (JSON, InDesign text)", &["json", "txt"])
            } else if request.purpose.starts_with("relink") {
                rfd::AsyncFileDialog::new().add_filter("Graphics", GRAPHIC_EXTS)
            } else if request.purpose == "place" {
                rfd::AsyncFileDialog::new().add_filter("Graphics", IMAGE_EXTS)
            } else {
                rfd::AsyncFileDialog::new().add_filter("Publisher, DesignCraft, IDML or InDesign", DOC_EXTS)
            };
            // Relink to Folder: the browser's folder picker hands over every graphic in the
            // chosen folder; they arrive together, to relink in one step.
            if request.purpose == "relinkFiles" {
                wasm_bindgen_futures::spawn_local(async move {
                    let files: Vec<ImportedFile> = pick_folder_files(&request.wanted)
                        .await
                        .into_iter()
                        .map(|(name, bytes)| ImportedFile { request: request.clone(), name, bytes })
                        .collect();
                    if !files.is_empty() {
                        inbox.lock().unwrap_or_else(|e| e.into_inner()).extend(files);
                        ctx.request_repaint();
                    }
                });
                return;
            }
            wasm_bindgen_futures::spawn_local(async move {
                // Place takes several files, loaded into the place cursor in the order chosen.
                let handles = if request.purpose == "place" {
                    dialog.pick_files().await.unwrap_or_default()
                } else {
                    dialog.pick_file().await.into_iter().collect()
                };
                let mut files = Vec::with_capacity(handles.len());
                for file in handles {
                    let bytes = file.read().await;
                    files.push(ImportedFile { request: request.clone(), name: file.file_name(), bytes });
                }
                inbox.lock().unwrap_or_else(|e| e.into_inner()).extend(files);
                ctx.request_repaint();
            });
        })),
        write: Some(Box::new(|name: &str, bytes: &[u8]| download(name, bytes))),
        download: Some(Box::new(|name: &str, bytes: &[u8]| download(name, bytes))),
        inbox: Some(inbox),
        ..Default::default()
    }
}

/// The most files a chosen folder hands over for relinking, and the most bytes in all.
const MAX_FOLDER_FILES: usize = 2000;
// The bytes are copied a few times on the way (base64 for the command, then decoded), so the
// cap keeps the peak well inside the browser's memory for a tab.
const MAX_FOLDER_BYTES: f64 = 256.0 * 1024.0 * 1024.0;

/// The graphics named in `wanted` (lower-case file names; empty: every graphic) in a folder the
/// user chooses with the browser's folder picker (an `<input type=file webkitdirectory>`, which
/// every current browser supports), each named by its path in the folder. Only those files are
/// read, up to [`MAX_FOLDER_FILES`] and [`MAX_FOLDER_BYTES`]; empty when the picker is cancelled
/// or can't be shown.
async fn pick_folder_files(wanted: &[String]) -> Vec<(String, Vec<u8>)> {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return Vec::new() };
    let Some(input) = document.create_element("input").ok().and_then(|e| e.dyn_into::<web_sys::HtmlInputElement>().ok()) else {
        return Vec::new();
    };
    input.set_type("file");
    input.set_multiple(true);
    if input.set_attribute("webkitdirectory", "").is_err() {
        return Vec::new();
    }
    // Settles when files are chosen or the picker is cancelled. The listener is this call's and
    // goes when the picker settles, not kept for the page's life. (A browser that fires neither
    // event leaves this call waiting; current browsers fire `cancel`.)
    let mut resolve = None;
    let chosen = js_sys::Promise::new(&mut |r, _reject| resolve = Some(r));
    let Some(resolve) = resolve else { return Vec::new() };
    let done = wasm_bindgen::closure::Closure::<dyn FnMut()>::new(move || {
        let _ = resolve.call0(&wasm_bindgen::JsValue::UNDEFINED);
    });
    for event in ["change", "cancel"] {
        let _ = input.add_event_listener_with_callback(event, done.as_ref().unchecked_ref());
    }
    input.click();
    let _ = wasm_bindgen_futures::JsFuture::from(chosen).await;
    for event in ["change", "cancel"] {
        let _ = input.remove_event_listener_with_callback(event, done.as_ref().unchecked_ref());
    }
    drop(done);
    let Some(list) = input.files() else { return Vec::new() };
    let mut out = Vec::new();
    let mut bytes = 0.0;
    for i in 0..list.length() {
        let Some(file) = list.get(i) else { continue };
        let name = file.name();
        let graphic = name.rsplit_once('.').is_some_and(|(_, ext)| GRAPHIC_EXTS.iter().any(|e| e.eq_ignore_ascii_case(ext)));
        if !graphic || !(wanted.is_empty() || wanted.contains(&name.to_lowercase())) {
            continue;
        }
        if out.len() >= MAX_FOLDER_FILES || bytes + file.size() > MAX_FOLDER_BYTES {
            log::warn!("Relink to Folder: more graphics than can be read at once; the rest are left");
            break;
        }
        bytes += file.size();
        // Its path within the chosen folder tells same-named files in different folders apart.
        let path = js_sys::Reflect::get(&file, &wasm_bindgen::JsValue::from_str("webkitRelativePath"))
            .ok()
            .and_then(|v| v.as_string())
            .filter(|p| !p.is_empty())
            .unwrap_or(name);
        if let Ok(buf) = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
            out.push((path, js_sys::Uint8Array::new(&buf).to_vec()));
        }
    }
    out
}

/// Trigger a browser download of `bytes` named after the last component of `path`.
fn download(path: &str, bytes: &[u8]) -> Result<(), String> {
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "foliolum".into());
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime_for(&name));
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(js)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js)?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a").map_err(js)?.dyn_into().map_err(|_| "not an anchor")?;
    a.set_href(&url);
    a.set_download(&name);
    a.style().set_property("display", "none").map_err(js)?;
    let body = document.body().ok_or("no body")?;
    body.append_child(&a).map_err(js)?;
    a.click();
    a.remove();
    // Revoke after the click has been dispatched; the download keeps its own reference.
    let revoke = wasm_bindgen::closure::Closure::once_into_js(move || {
        web_sys::Url::revoke_object_url(&url).ok();
    });
    // The download has started; a missed revoke only keeps the blob until the page closes.
    if let Err(e) = window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 10_000) {
        log::warn!("could not schedule revoking the download URL: {}", js(e));
    }
    Ok(())
}
