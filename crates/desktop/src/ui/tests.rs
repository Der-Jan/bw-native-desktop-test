use super::*;
use crate::{
    core::model::{Request, VaultCache},
    domain::UserId,
    services::sdk::test_support::unlocked_manager,
    test_support::fixtures::*,
};
use bitwarden_vault::{CipherId, CipherRepromptType};
fn setup() -> (egui::Context, Application) {
    crate::test_support::init();
    let ctx = egui::Context::default();
    let mut fonts = egui::FontDefinitions::empty();
    fonts.font_data.insert(
        "test".into(),
        egui::FontData::from_static(crate::assets::FONT_MEDIUM).into(),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.insert(family, vec!["test".into()]);
    }
    ctx.set_fonts(fonts);
    egui_extras::install_image_loaders(&ctx);
    let uid = UserId::new_v4();
    let mut core = Core::test(unlocked_manager(uid));
    let all = (0..50)
        .map(|i| {
            let mut item = make_cipher(CipherSpec {
                name: if i == 0 { "First" } else { "Next" },
                ..Default::default()
            });
            Arc::get_mut(&mut item).unwrap().id = Some(CipherId::new_v4());
            item
        })
        .collect::<Vec<_>>();
    core.caches.insert(
        uid,
        VaultCache {
            visible: all.clone(),
            all,
            ..Default::default()
        },
    );
    let app = Application::from_core(core, "Test".into());
    (ctx, app)
}
fn key(key: Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}
fn frame(ctx: &egui::Context, app: &mut Application, events: Vec<egui::Event>) {
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1250., 840.),
            )),
            events,
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
}
#[tokio::test]
async fn list_navigation_is_focus_scoped_and_escape_preserves_cursor() {
    let (ctx, mut app) = setup();
    frame(&ctx, &mut app, vec![]);
    ctx.memory_mut(|m| m.request_focus(Id::unique("vault-list-focus")));
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![key(Key::ArrowDown)]);
    let first = app.core.selected;
    assert!(first.is_some());
    frame(&ctx, &mut app, vec![key(Key::ArrowDown)]);
    let second = app.core.selected;
    assert_ne!(first, second);
    frame(&ctx, &mut app, vec![key(Key::Escape)]);
    assert!(!app.core.detail_open);
    assert_eq!(app.core.selected, second);
    ctx.memory_mut(|m| m.request_focus(Id::unique("vault-search")));
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![key(Key::ArrowDown)]);
    assert_eq!(app.core.selected, second);
}
#[tokio::test]
async fn page_keys_move_by_visible_page() {
    let (ctx, mut app) = setup();
    frame(&ctx, &mut app, vec![]);
    ctx.memory_mut(|m| m.request_focus(Id::unique("vault-list-focus")));
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![key(Key::PageDown)]);
    let position = app
        .core
        .current_cache()
        .unwrap()
        .visible
        .iter()
        .position(|v| v.id == app.core.selected)
        .unwrap();
    assert!(position > 1);
}
#[tokio::test]
async fn master_password_modal_gets_focus_and_escape_cancels() {
    let (ctx, mut app) = setup();
    let uid = app.core.active.unwrap();
    let item = Arc::get_mut(&mut app.core.caches.get_mut(&uid).unwrap().all[0]);
    assert!(item.is_none(), "shared projection is immutable");
    let id = app.core.current_cache().unwrap().all[0].id.unwrap();
    let mut value = serde_json::to_value(&*app.core.current_cache().unwrap().all[0]).unwrap();
    value["reprompt"] = serde_json::json!(CipherRepromptType::Password);
    let protected: Arc<bitwarden_vault::CipherListView> =
        Arc::new(serde_json::from_value(value).unwrap());
    let cache = app.core.caches.get_mut(&uid).unwrap();
    cache.all[0] = protected.clone();
    cache.visible[0] = protected;
    app.core.request(Request::Read(id));
    frame(&ctx, &mut app, vec![]);
    assert!(app.core.prompt.is_some());
    frame(&ctx, &mut app, vec![egui::Event::Text("wrong".into())]);
    assert_eq!(app.password.as_str(), "wrong");
    frame(&ctx, &mut app, vec![key(Key::Enter)]);
    assert!(app.core.prompt.as_ref().unwrap().busy);
    frame(&ctx, &mut app, vec![key(Key::Escape)]);
    assert!(app.core.prompt.is_none());
    assert!(app.password.is_empty());
}
#[tokio::test]
async fn lock_scrubs_input_draft_and_widget_memory() {
    let (ctx, mut app) = setup();
    app.password = Zeroizing::new("synthetic password".into());
    app.code = Zeroizing::new("123456".into());
    app.core.new_item(bitwarden_vault::CipherType::Login);
    frame(&ctx, &mut app, vec![]);
    app.core.lock_all();
    frame(&ctx, &mut app, vec![]);
    assert!(app.password.is_empty() && app.code.is_empty());
    assert!(app.core.editor.is_none());
    assert_ne!(
        ctx.memory(|m| m.focused()),
        Some(Id::unique("vault-list-focus"))
    );
}
#[test]
fn username_schemas_remain_compatible_with_sdk() {
    for request in [
        serde_json::json!({"subaddress":{"type":"random","email":"test@example.invalid"}}),
        serde_json::json!({"catchall":{"type":"random","domain":"example.invalid"}}),
    ] {
        assert!(
            serde_json::from_value::<bitwarden_generators::UsernameGeneratorRequest>(request)
                .is_ok()
        );
    }
}
#[tokio::test]
async fn tray_surface_close_is_not_cancelled_as_a_user_close() {
    let (ctx, mut app) = setup();
    app.core.hidden = true;
    let mut raw = egui::RawInput::default();
    raw.viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .events
        .push(egui::ViewportEvent::Close);
    let mut output = ctx.run_ui(raw, |ui| app.show(ui));
    assert!(
        !output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    assert!(app.core.hidden);
    assert!(!app.core.quit_requested);
    output.textures_delta.clear();
}

#[tokio::test]
async fn clicking_the_item_name_selects_it_and_opens_details() {
    let (ctx, mut app) = setup();
    frame(&ctx, &mut app, vec![]);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1250., 840.),
            )),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    let label = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "First" => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            _ => None,
        })
        .expect("first list label rendered");
    output.textures_delta.clear();
    let first = app.core.current_cache().unwrap().visible[0].id;
    for pressed in [true, false] {
        frame(
            &ctx,
            &mut app,
            vec![
                egui::Event::PointerMoved(label),
                egui::Event::PointerButton {
                    pos: label,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(
        app.core.selected, first,
        "clicking text must reach the row selection"
    );
    assert!(app.core.detail_open);
}

#[tokio::test]
async fn unlock_cleanup_preserves_typography_and_system_light_palette() {
    use crate::services::desktop_appearance::{Appearance, Palette};
    let (ctx, mut app) = setup();
    let palette = Palette::fallback(false, [61, 174, 233]);
    app.core.appearance = Appearance {
        accent: palette.selection,
        system_dark: Some(false),
        palette: Some(palette),
    };
    ctx.style_mut_of(egui::Theme::Light, |s| {
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(13.));
    });
    frame(&ctx, &mut app, vec![]);
    app.password = Zeroizing::new("synthetic secret".into());
    app.core.lock_all();
    frame(&ctx, &mut app, vec![]);
    assert!(app.password.is_empty());
    assert_eq!(ctx.theme(), egui::Theme::Light);
    assert_eq!(
        ctx.global_style().text_styles[&egui::TextStyle::Body].size,
        13.
    );
    assert_eq!(
        ctx.global_style().visuals.panel_fill,
        egui::Color32::from_rgb(239, 240, 241)
    );
}
