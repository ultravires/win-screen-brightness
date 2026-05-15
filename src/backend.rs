use brightness::blocking::{Brightness, BrightnessDevice};

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
            let gamma = GammaBrightness::new();
            let level = gamma.get();
            return (
                Self::Software(gamma),
                level,
                "软件伽马 · 外接屏需在显示器菜单中开启 DDC/CI 才能用硬件控制".into(),
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
        match self {
            Self::Hardware(dev) => dev.set(percent).map_err(|e| e.to_string()),
            #[cfg(windows)]
            Self::Software(g) => g.set(percent),
            #[cfg(not(windows))]
            Self::Unavailable => Err("无可用亮度控制".into()),
        }
    }
}

#[cfg(windows)]
pub struct GammaBrightness {
    saved_ramp: Option<[[u16; 256]; 3]>,
    level: u32,
}

#[cfg(windows)]
impl GammaBrightness {
    pub fn new() -> Self {
        Self {
            saved_ramp: capture_gamma_ramp(),
            level: 100,
        }
    }

    pub fn get(&self) -> u32 {
        self.level
    }

    pub fn set(&mut self, percent: u32) -> Result<(), String> {
        let percent = percent.min(100);
        apply_gamma_ramp(percent)?;
        self.level = percent;
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for GammaBrightness {
    fn drop(&mut self) {
        if let Some(ramp) = self.saved_ramp.take() {
            let _ = restore_gamma_ramp(&ramp);
        }
    }
}

#[cfg(windows)]
fn capture_gamma_ramp() -> Option<[[u16; 256]; 3]> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
    use windows::Win32::UI::ColorSystem::GetDeviceGammaRamp;

    unsafe {
        let hdc = GetDC(Some(HWND::default()));
        if hdc.is_invalid() {
            return None;
        }
        let mut ramp = [[0u16; 256]; 3];
        let ok = GetDeviceGammaRamp(hdc, &mut ramp as *mut _ as *mut _).as_bool();
        let _ = ReleaseDC(Some(HWND::default()), hdc);
        ok.then_some(ramp)
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

#[cfg(windows)]
fn restore_gamma_ramp(ramp: &[[u16; 256]; 3]) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{GetDC, ReleaseDC};
    use windows::Win32::UI::ColorSystem::SetDeviceGammaRamp;

    unsafe {
        let hdc = GetDC(Some(HWND::default()));
        if hdc.is_invalid() {
            return Err("无法获取显示设备上下文".into());
        }
        SetDeviceGammaRamp(hdc, ramp as *const _ as *const _)
            .ok()
            .map_err(|e| e.to_string())?;
        let _ = ReleaseDC(Some(HWND::default()), hdc);
    }
    Ok(())
}
