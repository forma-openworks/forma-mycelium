// В релизной сборке под Windows не показываем консольное окно.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod editors;
mod fonts;
mod icon;
mod project;
mod settings;
mod theme;
mod widgets;
mod workspace;

use eframe::{egui, egui_wgpu, wgpu};
use mycelium_i18n::t;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    // Проект можно передать аргументом: `forma-mycelium "D:\Моды\my-mod"`.
    let project_arg = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    let settings = settings::Settings::load();
    mycelium_i18n::init(settings::user_locales_dir().as_deref());
    mycelium_i18n::set_language(&settings.effective_language());

    // Разрешаем только Vulkan: весь интерфейс рисуется через Vulkan-бэкенд wgpu.
    let mut setup = egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = wgpu::Backends::VULKAN;
    setup.power_preference = wgpu::PowerPreference::HighPerformance;

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_title(app::APP_TITLE)
            .with_app_id("forma-mycelium")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([800.0, 520.0])
            .with_icon(theme::app_icon()),
        wgpu_options: egui_wgpu::WgpuConfiguration { wgpu_setup: setup.into(), ..Default::default() },
        // Размер и положение окна eframe хранит рядом с остальными настройками.
        persistence_path: settings::config_dir().map(|d| d.join("window")),
        ..Default::default()
    };

    let result = eframe::run_native(
        app::APP_TITLE,
        options,
        Box::new(move |cc| Ok(Box::new(app::MyceliumApp::new(cc, settings, project_arg)))),
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
