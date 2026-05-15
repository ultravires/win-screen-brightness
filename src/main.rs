use brightness::blocking::{Brightness, BrightnessDevice};
use eframe::egui;

/// 加载支持中文的系统字体，避免 egui 默认字体显示为方框。
fn setup_cjk_fonts(ctx: &egui::Context) {
    let font_bytes = load_cjk_font_bytes();
    let Some(font_bytes) = font_bytes else {
        eprintln!("警告: 未找到中文字体，界面中文可能无法正常显示");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "cjk".to_owned(),
        egui::FontData::from_owned(font_bytes).into(),
    );

    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "cjk".to_owned());
    }

    ctx.set_fonts(fonts);
}

fn load_cjk_font_bytes() -> Option<Vec<u8>> {
    #[cfg(windows)]
    const CANDIDATES: &[&str] = &[
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\msyhbd.ttc",
        r"C:\Windows\Fonts\simhei.ttf",
        r"C:\Windows\Fonts\simsun.ttc",
    ];

    #[cfg(target_os = "macos")]
    const CANDIDATES: &[&str] = &[
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/Library/Fonts/Arial Unicode.ttf",
    ];

    #[cfg(all(unix, not(target_os = "macos")))]
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ];

    #[cfg(not(any(windows, target_os = "macos", all(unix, not(target_os = "macos")))))]
    const CANDIDATES: &[&str] = &[];

    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            return Some(bytes);
        }
    }
    None
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 200.0])
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        "屏幕亮度调节",
        options,
        Box::new(|cc| {
            setup_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(BrightnessApp::default()))
        }),
    )
}

struct BrightnessApp {
    brightness: u32,          // 当前亮度 (0-100)
    devices: Vec<BrightnessDevice>,
    status: String,
}

impl Default for BrightnessApp {
    fn default() -> Self {
        let devices: Vec<BrightnessDevice> = brightness::blocking::brightness_devices()
            .filter_map(Result::ok)
            .collect();

        let mut app = Self {
            brightness: 50,
            devices,
            status: String::new(),
        };

        // 初始化读取当前亮度
        if let Some(dev) = app.devices.first() {
            if let Ok(val) = dev.get() {
                app.brightness = val;
            }
        }

        app
    }
}

impl eframe::App for BrightnessApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("屏幕亮度调节");
            ui.add_space(20.0);

            if self.devices.is_empty() {
                ui.label("未检测到亮度设备（可能不支持或权限问题）");
                return;
            }

            // 滑块
            let response = ui.add(
                egui::Slider::new(&mut self.brightness, 0..=100)
                    .text("亮度 (%)")
                    .step_by(1.0)
            );

            if response.changed() {
                self.set_brightness();
            }

            ui.add_space(10.0);
            ui.label(format!("当前亮度: {}%", self.brightness));

            if !self.status.is_empty() {
                ui.colored_label(egui::Color32::YELLOW, &self.status);
            }

            ui.separator();

            if ui.button("刷新设备").clicked() {
                self.devices = brightness::blocking::brightness_devices()
                    .filter_map(Result::ok)
                    .collect();
                self.status = format!("找到 {} 个设备", self.devices.len());
            }

            ui.label(format!("检测到设备数量: {}", self.devices.len()));
        });
    }
}

impl BrightnessApp {
    fn set_brightness(&mut self) {
        if let Some(dev) = self.devices.first() {
            match dev.set(self.brightness) {
                Ok(_) => {
                    self.status = format!("已设置为 {}%", self.brightness);
                }
                Err(e) => {
                    self.status = format!("设置失败: {}", e);
                }
            }
        } else {
            self.status = "没有可用设备".to_string();
        }
    }
}