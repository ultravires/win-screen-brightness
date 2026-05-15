use brightness::blocking::{Brightness, BrightnessDevice};
use std::path::PathBuf;

/// 亮度控制后端：优先硬件（DDC/CI 或笔记本面板 IOCTL），否则在 Windows 上使用伽马曲线。
pub enum BrightnessBackend {
    Hardware(BrightnessDevice),
    #[cfg(windows)]
    Software(GammaBrightness),
    #[cfg(not(windows))]
    Unavailable,
}

impl BrightnessBackend {
    /// 探测可用后端并读取当前亮度。
    pub fn discover() -> (Self, u32, String) {
        for dev in brightness::blocking::brightness_devices().filter_map(Result::ok) {
            if let Ok(level) = dev.get() {
                let label = dev
                    .friendly_device_name()
                    .unwrap_or_else(|_| "未知显示器".into());
                return (Self::Hardware(dev), level, format!("硬件控制 · {label}"));
            }
        }

        #[cfg(windows)]
        {
            let mut gamma = GammaBrightness::new();
            let level = load_saved_brightness().unwrap_or(100);
            let _ = gamma.set(level);
            return (
                Self::Software(gamma),
                level,
                "软件伽马 · 退出后仍保持（重启或注销后恢复）· 开启 DDC/CI 可改用硬件控制".into(),
            );
        }

        #[cfg(not(windows))]
        return (
            Self::Unavailable,
            50,
            "未找到可用的亮度设备".into(),
        );
    }

    pub fn set(&mut self, percent: u32) -> Result<(), String> {
        let percent = percent.min(100);
        let result = match self {
            Self::Hardware(dev) => dev.set(percent).map_err(|e| e.to_string()),
            #[cfg(windows)]
            Self::Software(g) => g.set(percent),
            #[cfg(not(windows))]
            Self::Unavailable => Err("无可用亮度控制".into()),
        };
        if result.is_ok() {
            save_brightness(percent);
        }
        result
    }
}

fn config_path() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|base| {
            PathBuf::from(base)
                .join("screen-brightness")
                .join("brightness.txt")
        })
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(|base| {
            PathBuf::from(base)
                .join(".config")
                .join("screen-brightness")
                .join("brightness.txt")
        })
    }
}

fn load_saved_brightness() -> Option<u32> {
    let text = std::fs::read_to_string(config_path()?).ok()?;
    let v: u32 = text.trim().parse().ok()?;
    Some(v.min(100))
}

fn save_brightness(percent: u32) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, percent.to_string());
}

#[cfg(windows)]
pub struct GammaBrightness {
    level: u32,
}

#[cfg(windows)]
impl GammaBrightness {
    pub fn new() -> Self {
        Self { level: 100 }
    }

    pub fn set(&mut self, percent: u32) -> Result<(), String> {
        let percent = percent.min(100);
        apply_gamma_ramp(percent)?;
        self.level = percent;
        Ok(())
    }
}

#[cfg(windows)]
fn apply_gamma_ramp(percent: u32) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
    use windows::Win32::UI::ColorSystem::SetDeviceGammaRamp;

    let mut ramp = [[0u16; 256]; 3];
    for i in 0..256 {
        let channel = (i as u32 * 65535 * percent / (255 * 100)).min(65535) as u16;
        ramp[0][i] = channel;
        ramp[1][i] = channel;
        ramp[2][i] = channel;
    }

    unsafe {
        let hdc = GetDC(Some(HWND::default()));
        if hdc.is_invalid() {
            return Err("无法获取显示设备上下文".into());
        }
        SetDeviceGammaRamp(hdc, &ramp as *const _ as *const _)
            .ok()
            .map_err(|e| e.to_string())?;
        let _ = ReleaseDC(Some(HWND::default()), hdc);
    }
    Ok(())
}

