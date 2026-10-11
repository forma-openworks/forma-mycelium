// В релизной сборке под Windows не показываем консольное окно.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod actions;
mod app;
mod colors;
mod discord;
mod editors;
mod highlight;
mod icon;
mod lsp;
mod mods_view;
mod plugins;
mod run;
mod settings;
mod sync;
mod workspace;

use eframe::{egui, egui_wgpu, wgpu};
use forma_i18n::t;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // Проект можно передать аргументом: `forma-mycelium "D:\Моды\my-mod"`.
    let project_arg = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    let mut store = forma_settings::Store::new(settings::config_dir());
    let settings = settings::Settings::load(&mut store);
    forma_i18n::init(settings::user_locales_dir().as_deref());
    forma_i18n::set_language(&settings.appearance.effective_language());

    // Разрешаем только Vulkan: весь интерфейс рисуется через Vulkan-бэкенд wgpu.
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::VULKAN;
    setup.power_preference = wgpu::PowerPreference::HighPerformance;

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title(app::APP_TITLE)
            .with_app_id("forma-mycelium")
            // Системной рамки нет: шапку с кнопками окна рисует сама программа.
            .with_decorations(false)
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([800.0, 520.0])
            .with_icon(app::app_icon()),
        wgpu_options: egui_wgpu::WgpuConfiguration { wgpu_setup: setup.into(), ..Default::default() },
        // Размер и положение окна eframe хранит рядом с остальными настройками.
        persistence_path: settings::config_dir().map(|d| d.join("window")),
        ..Default::default()
    };

    let result = eframe::run_native(
        app::APP_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(app::MyceliumApp::new(cc, settings, store, project_arg)))),
    );

    if let Err(err) = result {
        let text = t!("error-vulkan", details = err.to_string());
        log::error!("{text}");
        rfd::MessageDialog::new()
            .set_title(app::APP_TITLE)
            .set_description(&text)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}
